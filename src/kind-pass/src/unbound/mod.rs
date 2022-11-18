//! Collects all the unbound variables and
//! check if patterns are linear.
//!
//! It also gets all of the identifiers used
//! by sugars because it's useful to name resolution
//! phase.

use std::sync::mpsc::Sender;

use fxhash::{FxHashMap, FxHashSet};
use kind_report::data::Diagnostic;
use kind_span::Range;
use kind_tree::concrete::expr::{Binding, Case, CaseBinding, Destruct};
use kind_tree::concrete::pat::PatIdent;
use kind_tree::concrete::{Book, Module, TopLevel};
use kind_tree::symbol::{Ident, QualifiedIdent};

use kind_tree::concrete::{
    expr::{Expr, ExprKind, SttmKind},
    pat::{Pat, PatKind},
    visitor::Visitor,
    Argument, Entry, Rule,
};
use kind_tree::{visit_opt, visit_vec};

use crate::errors::PassError;

pub struct UnboundCollector {
    pub errors: Sender<Box<dyn Diagnostic>>,

    // Utils for keeping variables tracking and report duplicated ones.
    pub context_vars: Vec<(Range, String)>,

    pub top_level_defs: FxHashMap<String, Range>,
    pub unbound_top_level: FxHashMap<String, FxHashSet<QualifiedIdent>>,
    pub unbound: FxHashMap<String, Vec<Ident>>,
    pub emit_errs: bool,
}

impl UnboundCollector {
    pub fn new(
        diagnostic_sender: Sender<Box<dyn Diagnostic>>,
        emit_errs: bool,
    ) -> UnboundCollector {
        Self {
            errors: diagnostic_sender,
            context_vars: Default::default(),
            top_level_defs: Default::default(),
            unbound_top_level: Default::default(),
            unbound: Default::default(),
            emit_errs,
        }
    }
}

pub fn get_module_unbound(
    diagnostic_sender: Sender<Box<dyn Diagnostic>>,
    module: &mut Module,
    emit_errs: bool,
) -> (
    FxHashMap<String, Vec<Ident>>,
    FxHashMap<String, FxHashSet<QualifiedIdent>>,
) {
    let mut state = UnboundCollector::new(diagnostic_sender, emit_errs);
    state.visit_module(module);
    (state.unbound, state.unbound_top_level)
}

pub fn get_book_unbound(
    diagnostic_sender: Sender<Box<dyn Diagnostic>>,
    book: &mut Book,
    emit_errs: bool,
) -> (
    FxHashMap<String, Vec<Ident>>,
    FxHashMap<String, FxHashSet<QualifiedIdent>>,
) {
    let mut state = UnboundCollector::new(diagnostic_sender, emit_errs);
    state.visit_book(book);
    (state.unbound, state.unbound_top_level)
}

impl UnboundCollector {
    fn visit_top_level_names(&mut self, toplevel: &mut TopLevel) {
        match toplevel {
            TopLevel::SumType(sum) => {
                self.top_level_defs
                    .insert(sum.name.to_string(), sum.name.range);
                for cons in &sum.constructors {
                    let name_cons = sum.name.add_segment(cons.name.to_str());
                    self.top_level_defs
                        .insert(name_cons.to_string(), name_cons.range);
                }
            }
            TopLevel::RecordType(rec) => {
                self.top_level_defs
                    .insert(rec.name.to_string(), rec.name.range);
                let name_cons = rec.name.add_segment(rec.constructor.to_str());
                self.top_level_defs
                    .insert(name_cons.to_string(), name_cons.range);
            }
            TopLevel::Entry(entry) => {
                self.top_level_defs
                    .insert(entry.name.to_string(), entry.name.range);
            }
        }
    }
}

impl Visitor for UnboundCollector {
    fn visit_attr(&mut self, _: &mut kind_tree::concrete::Attribute) {}

    fn visit_ident(&mut self, ident: &mut Ident) {
        let name = ident.to_str();
        if self
            .context_vars
            .iter()
            .all(|x| x.1 != name)
        {
            let entry = self
                .unbound
                .entry(name.to_string())
                .or_insert_with(Vec::new);
            entry.push(ident.clone());
        }
    }

    fn visit_qualified_ident(&mut self, ident: &mut QualifiedIdent) {
        if !self.top_level_defs.contains_key(&ident.to_string()) {
            let entry = self.unbound_top_level.entry(ident.to_string()).or_default();
            entry.insert(ident.clone());
        }
    }

    fn visit_pat_ident(&mut self, ident: &mut PatIdent) {
        let name = ident.0.to_str();
        if let Some(fst) = self
            .context_vars
            .iter()
            .find(|x| x.1 == name)
        {
            if self.emit_errs {
                self.errors
                    .send(Box::new(PassError::RepeatedVariable(fst.0, ident.0.range)))
                    .unwrap()
            }
        } else {
            self.context_vars.push((ident.0.range, name.to_string()))
        }
    }

    fn visit_argument(&mut self, argument: &mut Argument) {
        match &mut argument.typ {
            Some(res) => self.visit_expr(res),
            None => (),
        }

        let res = self
            .context_vars
            .iter()
            .find(|x| x.1 == argument.name.to_str());

        if let Some(fst) = res {
            if self.emit_errs {
                self.errors
                    .send(Box::new(PassError::RepeatedVariable(
                        fst.0,
                        argument.name.range,
                    )))
                    .unwrap()
            }
        } else {
            self.context_vars
                .push((argument.name.range, argument.name.to_string()))
        }

        self.context_vars
            .push((argument.name.range, argument.name.to_string()));
    }

    fn visit_rule(&mut self, rule: &mut Rule) {
        let vars = self.context_vars.clone();
        for pat in &mut rule.pats {
            self.visit_pat(pat);
        }
        self.visit_expr(&mut rule.body);
        self.context_vars = vars;
    }

    fn visit_entry(&mut self, entry: &mut Entry) {
        let vars = self.context_vars.clone();

        for arg in entry.args.iter_mut() {
            self.visit_argument(arg)
        }

        self.visit_expr(&mut entry.typ);

        self.context_vars = vars;

        for rule in &mut entry.rules {
            self.visit_rule(rule)
        }
    }

    fn visit_top_level(&mut self, toplevel: &mut TopLevel) {
        match toplevel {
            TopLevel::SumType(entr) => {
                let mut repeated_names = FxHashMap::<String, Range>::default();
                let mut failed = false;

                for cons in &entr.constructors {
                    match repeated_names.get(&cons.name.to_string()) {
                        Some(_) => {
                            failed = true;
                        }
                        None => {
                            repeated_names.insert(cons.name.to_string(), cons.name.range);
                        }
                    }
                    let name_cons = entr.name.add_segment(cons.name.to_str());
                    self.context_vars
                        .push((name_cons.range, name_cons.to_string()));
                }

                if failed {
                    return;
                }

                let vars = self.context_vars.clone();

                visit_vec!(entr.parameters.iter_mut(), arg => self.visit_argument(arg));

                let inside_vars = self.context_vars.clone();

                visit_vec!(entr.indices.iter_mut(), arg => self.visit_argument(arg));

                visit_vec!(entr.constructors.iter_mut(), cons => {
                    self.context_vars = inside_vars.clone();
                    visit_vec!(cons.args.iter_mut(), arg => self.visit_argument(arg));
                    visit_opt!(&mut cons.typ, arg => self.visit_expr(arg));
                });

                self.context_vars = vars;
            }
            TopLevel::RecordType(entr) => {
                let inside_vars = self.context_vars.clone();
                visit_vec!(entr.parameters.iter_mut(), arg => self.visit_argument(arg));
                visit_vec!(entr.fields.iter_mut(), (_, _, typ) => {
                    self.visit_expr(typ);
                });

                self.context_vars = inside_vars;
            }
            TopLevel::Entry(entr) => self.visit_entry(entr),
        }
    }

    fn visit_module(&mut self, book: &mut kind_tree::concrete::Module) {
        for entr in &mut book.entries {
            self.visit_top_level_names(entr);
        }
        for entr in &mut book.entries {
            self.visit_top_level(entr)
        }
    }

    fn visit_book(&mut self, book: &mut Book) {
        for entr in book.entries.values_mut() {
            self.visit_top_level_names(entr);
        }
        for entr in book.entries.values_mut() {
            self.visit_top_level(entr)
        }
    }

    fn visit_destruct(&mut self, destruct: &mut Destruct) {
        match destruct {
            Destruct::Destruct(range, ty, bindings, _) => {
                self.visit_qualified_ident(
                    &mut QualifiedIdent::add_segment(ty, "open")
                        .to_sugar()
                        .to_generated(),
                );
                self.visit_range(range);
                self.visit_qualified_ident(ty);
                for bind in bindings {
                    self.visit_case_binding(bind)
                }
            }
            Destruct::Ident(ident) => self.context_vars.push((ident.range, ident.to_string())),
        }
    }

    fn visit_sttm(&mut self, sttm: &mut kind_tree::concrete::expr::Sttm) {
        match &mut sttm.data {
            SttmKind::Ask(ident, val, next) => {
                self.visit_expr(val);
                let vars = self.context_vars.clone();
                self.visit_destruct(ident);
                self.visit_sttm(next);
                self.context_vars = vars;
            }
            SttmKind::Let(ident, val, next) => {
                self.visit_expr(val);
                let vars = self.context_vars.clone();
                self.visit_destruct(ident);
                self.visit_sttm(next);
                self.context_vars = vars;
            }
            SttmKind::Expr(expr, next) => {
                self.visit_expr(expr);
                self.visit_sttm(next);
            }
            SttmKind::Return(expr) => {
                self.visit_expr(expr);
            }
            SttmKind::RetExpr(expr) => {
                self.visit_expr(expr);
            }
        }
    }

    fn visit_pat(&mut self, pat: &mut Pat) {
        match &mut pat.data {
            PatKind::Var(ident) => self.visit_pat_ident(ident),
            PatKind::Str(_) => (),
            PatKind::Num(_) => (),
            PatKind::Hole => (),
            PatKind::List(ls) => {
                for pat in ls {
                    self.visit_pat(pat)
                }
            }
            PatKind::Pair(fst, snd) => {
                self.visit_pat(fst);
                self.visit_pat(snd);
            }
            PatKind::App(t, ls) => {
                self.visit_qualified_ident(t);
                for pat in ls {
                    self.visit_pat(pat)
                }
            }
        }
    }

    fn visit_case_binding(&mut self, case_binding: &mut CaseBinding) {
        match case_binding {
            CaseBinding::Field(ident) | CaseBinding::Renamed(_, ident) => {
                self.context_vars.push((ident.range, ident.to_string()))
            }
        }
    }

    fn visit_case(&mut self, case: &mut Case) {
        let vars = self.context_vars.clone();
        for binding in &mut case.bindings {
            self.visit_case_binding(binding);
        }
        self.visit_expr(&mut case.value);
        self.context_vars = vars;
    }

    fn visit_match(&mut self, matcher: &mut kind_tree::concrete::expr::Match) {
        self.visit_expr(&mut matcher.scrutinizer);
        for case in &mut matcher.cases {
            self.visit_case(case);
        }
        match &mut matcher.motive {
            Some(x) => self.visit_expr(x),
            None => (),
        }
    }

    fn visit_binding(&mut self, binding: &mut Binding) {
        match binding {
            Binding::Positional(e) => self.visit_expr(e),
            Binding::Named(_, _, e) => self.visit_expr(e),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match &mut expr.data {
            ExprKind::Var(ident) => self.visit_ident(ident),
            ExprKind::Constr(ident, spine) => {
                self.visit_qualified_ident(ident);
                visit_vec!(spine.iter_mut(), arg => self.visit_binding(arg));
            }
            ExprKind::All(None, typ, body) => {
                self.visit_expr(typ);
                self.visit_expr(body);
            }
            ExprKind::All(Some(ident), typ, body) => {
                self.visit_expr(typ);
                self.context_vars.push((ident.range, ident.to_string()));
                self.visit_expr(body);
                self.context_vars.pop();
            }
            ExprKind::Lambda(ident, binder, body, _erased) => {
                match binder {
                    Some(x) => self.visit_expr(x),
                    None => (),
                }
                self.context_vars.push((ident.range, ident.to_string()));
                self.visit_expr(body);
                self.context_vars.pop();
            }
            ExprKind::App(head, spine) => {
                self.visit_expr(head);
                visit_vec!(spine.iter_mut(), arg => self.visit_expr(&mut arg.data));
            }
            ExprKind::Ann(val, ty) => {
                self.visit_expr(val);
                self.visit_expr(ty);
            }
            ExprKind::Lit(lit) => self.visit_literal(lit),
            ExprKind::Binary(_, l, r) => {
                self.visit_expr(l);
                self.visit_expr(r);
            }
            ExprKind::Let(ident, val, body) => {
                self.visit_expr(val);
                let vars = self.context_vars.clone();
                self.visit_destruct(ident);
                self.visit_expr(body);
                self.context_vars = vars;
            }
            ExprKind::Sigma(None, typ, body) => {
                self.visit_qualified_ident(
                    QualifiedIdent::new_static("Sigma", None, expr.range).to_sugar(),
                );
                self.visit_expr(typ);
                self.visit_expr(body);
            }
            ExprKind::Sigma(Some(ident), typ, body) => {
                self.visit_qualified_ident(
                    QualifiedIdent::new_static("Sigma", None, expr.range).to_sugar(),
                );
                self.visit_expr(typ);
                self.context_vars.push((ident.range, ident.to_string()));
                self.visit_expr(body);
                self.context_vars.pop();
            }
            ExprKind::Match(matcher) => {
                self.visit_qualified_ident(matcher.typ.add_segment("match").to_sugar());
                self.visit_match(matcher)
            }
            ExprKind::Subst(subst) => {
                self.visit_ident(&mut subst.name);

                if let Some(pos) = self
                    .context_vars
                    .iter()
                    .position(|x| x.1 == subst.name.to_string())
                {
                    subst.indx = pos;
                }

                self.visit_expr(&mut subst.expr)
            }
            ExprKind::Hole => {}
            ExprKind::Do(typ, sttm) => {
                self.visit_qualified_ident(typ.add_segment("pure").to_sugar());
                self.visit_qualified_ident(typ.add_segment("bind").to_sugar());
                self.visit_sttm(sttm)
            }
            ExprKind::If(cond, if_, else_) => {
                self.visit_qualified_ident(&mut QualifiedIdent::new_sugared(
                    "Bool", "if", expr.range,
                ));
                self.visit_expr(cond);
                self.visit_expr(if_);
                self.visit_expr(else_);
            }
            ExprKind::Pair(l, r) => {
                self.visit_qualified_ident(&mut QualifiedIdent::new_sugared(
                    "Pair", "new", expr.range,
                ));
                self.visit_expr(l);
                self.visit_expr(r);
            }
            ExprKind::List(spine) => {
                self.visit_qualified_ident(&mut QualifiedIdent::new_sugared(
                    "List", "nil", expr.range,
                ));
                self.visit_qualified_ident(&mut QualifiedIdent::new_sugared(
                    "List", "cons", expr.range,
                ));
                visit_vec!(spine.iter_mut(), arg => self.visit_expr(arg));
            }
        }
    }
}
