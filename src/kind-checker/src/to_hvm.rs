use core::fmt;

use kind_span::Span;
use kind_tree::desugared::{self, Expr, Glossary};
use kind_tree::Operator;

use hvm::Term;
use kind_tree::symbol::Ident;

use hvm::language as lang;

macro_rules! vec_preppend {
    ($($f:expr),*; $e:expr) => {
        vec![[$($f),*].as_slice(), &$e.as_slice()].concat()
    };
}

#[derive(Debug)]
pub enum TermTag {
    Var,
    All,
    Lambda,
    App,
    Fun(usize),
    Ctr(usize),
    Let,
    Ann,
    Sub,
    Typ,
    U60,
    Num,
    Binary,
    Hole,
    Hlp,

    // The HOAS Tags
    HoasF(String),
    HoasQ(String),
}

pub enum EvalTag {
    EvalOp,
    EvalApp,
    EvalLet,
    EvalAnn,
    EvalSub,
}

impl fmt::Display for TermTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TermTag::Var => write!(f, "Kind.Term.var"),
            TermTag::All => write!(f, "Kind.Term.all"),
            TermTag::Lambda => write!(f, "Kind.Term.lam"),
            TermTag::App => write!(f, "Kind.Term.app"),
            TermTag::Fun(n) => write!(f, "Kind.Term.fun{}", n),
            TermTag::Ctr(n) => write!(f, "Kind.Term.ctr{}", n),
            TermTag::Let => write!(f, "Kind.Term.let"),
            TermTag::Ann => write!(f, "Kind.Term.ann"),
            TermTag::Sub => write!(f, "Kind.Term.sub"),
            TermTag::Typ => write!(f, "Kind.Term.typ"),
            TermTag::U60 => write!(f, "Kind.Term.u60"),
            TermTag::Num => write!(f, "Kind.Term.num"),
            TermTag::Binary => write!(f, "Kind.Term.op2"),
            TermTag::Hole => write!(f, "Kind.Term.hol"),
            TermTag::Hlp => write!(f, "Kind.Term.hlp"),
            TermTag::HoasF(name) => write!(f, "F${}", name),
            TermTag::HoasQ(name) => write!(f, "Q${}", name),
        }
    }
}

impl fmt::Display for EvalTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalTag::EvalOp => write!(f, "Kind.Term.eval_op"),
            EvalTag::EvalApp => write!(f, "Kind.Term.eval_app"),
            EvalTag::EvalLet => write!(f, "Kind.Term.eval_let"),
            EvalTag::EvalAnn => write!(f, "Kind.Term.eval_ann"),
            EvalTag::EvalSub => write!(f, "Kind.Term.eval_sub"),
        }
    }
}

// Codegen

pub fn operator_to_constructor<'a>(operator: Operator) -> &'a str {
    match operator {
        Operator::Add => "Kind.Operator.add",
        Operator::Sub => "Kind.Operator.sub",
        Operator::Mul => "Kind.Operator.mul",
        Operator::Div => "Kind.Operator.div",
        Operator::Mod => "Kind.Operator.mod",
        Operator::And => "Kind.Operator.and",
        Operator::Xor => "Kind.Operator.xor",
        Operator::Shl => "Kind.Operator.shl",
        Operator::Shr => "Kind.Operator.shr",
        Operator::Ltn => "Kind.Operator.ltn",
        Operator::Lte => "Kind.Operator.lte",
        Operator::Eql => "Kind.Operator.eql",
        Operator::Gte => "Kind.Operator.gte",
        Operator::Gtn => "Kind.Operator.gtn",
        Operator::Neq => "Kind.Operator.neq",
        Operator::Or => "Kind.Operator.or",
    }
}

pub fn eval_ctr(quote: bool, head: TermTag) -> String {
    if !quote {
        head.to_string()
    } else {
        match head {
            TermTag::Binary => EvalTag::EvalOp.to_string(),
            TermTag::App => EvalTag::EvalApp.to_string(),
            TermTag::Let => EvalTag::EvalLet.to_string(),
            TermTag::Ann => EvalTag::EvalAnn.to_string(),
            TermTag::Sub => EvalTag::EvalSub.to_string(),
            other => other.to_string(),
        }
    }
}

// Helpers

pub fn lift_spine(spine: Vec<Box<Term>>) -> Vec<Box<Term>> {
    if spine.len() >= 14 {
        vec![Box::new(Term::Ctr {
            name: format!("Kind.Term.args{}", spine.len()),
            args: spine,
        })]
    } else {
        spine
    }
}

pub fn mk_quoted_ctr(head: String, spine: Vec<Box<Term>>) -> Box<Term> {
    let args = lift_spine(spine);
    Box::new(Term::Ctr { name: head, args })
}

pub fn mk_ctr(name: String, args: Vec<Box<Term>>) -> Box<Term> {
    Box::new(lang::Term::Ctr { name, args })
}

pub fn mk_var(ident: &str) -> Box<Term> {
    Box::new(Term::Var {
        name: ident.to_string(),
    })
}

pub fn mk_u60(numb: u64) -> Box<Term> {
    Box::new(Term::Num { numb })
}

pub fn mk_single_ctr(head: String) -> Box<Term> {
    Box::new(Term::Ctr {
        name: head,
        args: vec![],
    })
}

pub fn mk_ctr_name(ident: &Ident) -> Box<Term> {
    // Adds an empty segment (so it just appends a dot in the end)
    mk_single_ctr(ident.add_segment("").to_string())
}

pub fn span_to_num(span: Span) -> Box<Term> {
    Box::new(Term::Num {
        numb: span.encode().0,
    })
}

pub fn set_origin(ident: &Ident) -> Box<Term> {
    mk_quoted_ctr(
        "Kind.Term.set_origin".to_owned(),
        vec![
            span_to_num(Span::Locatable(ident.range)),
            mk_var(ident.to_str()),
        ],
    )
}

pub fn lam(name: &Ident, body: Box<Term>) -> Box<Term> {
    Box::new(Term::Lam {
        name: name.to_string(),
        body,
    })
}

pub fn codegen_str(input: &str) -> Box<Term> {
    input.chars().rfold(
        Box::new(Term::Ctr {
            name: "String.nil".to_string(),
            args: vec![],
        }),
        |right, chr| {
            Box::new(Term::Ctr {
                name: "String.cons".to_string(),
                args: vec![mk_u60(chr as u64), right],
            })
        },
    )
}

pub fn codegen_all_expr(lhs: bool, num: &mut usize, quote: bool, expr: &Expr) -> Box<Term> {
    use kind_tree::desugared::ExprKind::*;

    match &expr.data {
        Typ => mk_quoted_ctr(eval_ctr(quote, TermTag::Typ), vec![span_to_num(expr.span)]),
        U60 => mk_quoted_ctr(eval_ctr(quote, TermTag::U60), vec![span_to_num(expr.span)]),
        Var(ident) => {
            if lhs {
                *num += 1;
                mk_quoted_ctr(
                    eval_ctr(quote, TermTag::Var),
                    vec![
                        span_to_num(expr.span),
                        mk_u60(ident.encode()),
                        mk_u60((*num - 1) as u64),
                    ],
                )
            } else if quote {
                set_origin(ident)
            } else {
                mk_var(ident.to_str())
            }
        }
        All(name, typ, body) => {
            let name = name.clone().unwrap_or_else(|| Ident::generate("~"));
            mk_quoted_ctr(
                eval_ctr(quote, TermTag::All),
                vec![
                    span_to_num(expr.span),
                    mk_u60(name.encode()),
                    codegen_all_expr(lhs, num, quote, typ),
                    lam(&name, codegen_all_expr(lhs, num, quote, body)),
                ],
            )
        }
        Lambda(name, body) => mk_quoted_ctr(
            eval_ctr(quote, TermTag::Lambda),
            vec![
                span_to_num(expr.span),
                mk_u60(name.encode()),
                lam(name, codegen_all_expr(lhs, num, quote, body)),
            ],
        ),
        App(head, spine) => {
            spine
                .iter()
                .fold(codegen_all_expr(lhs, num, quote, head), |left, right| {
                    Box::new(Term::App {
                        func: left,
                        argm: codegen_all_expr(lhs, num, quote, &right.clone()),
                    })
                })
        }
        Ctr(name, spine) => mk_quoted_ctr(
            eval_ctr(quote, TermTag::Ctr(spine.len())),
            vec_preppend![
                mk_ctr_name(name),
                span_to_num(expr.span);
                spine.iter().cloned().map(|x| codegen_all_expr(lhs, num, quote, &x)).collect::<Vec<Box<Term>>>()
            ],
        ),
        Fun(name, spine) => {
            let spine = spine
                .iter()
                .cloned()
                .map(|x| codegen_all_expr(lhs, num, quote, &x))
                .collect::<Vec<Box<Term>>>();
            if quote {
                mk_quoted_ctr(
                    eval_ctr(quote, TermTag::Fun(spine.len())),
                    vec_preppend![
                        mk_ctr_name(name),
                        span_to_num(expr.span);
                        spine
                    ],
                )
            } else {
                mk_quoted_ctr(
                    TermTag::HoasF(name.to_string()).to_string(),
                    vec_preppend![
                        span_to_num(expr.span);
                        spine
                    ],
                )
            }
        }
        Let(name, val, body) => mk_quoted_ctr(
            eval_ctr(quote, TermTag::Let),
            vec![
                span_to_num(expr.span),
                mk_u60(name.encode()),
                codegen_all_expr(lhs, num, quote, val),
                lam(name, codegen_all_expr(lhs, num, quote, body)),
            ],
        ),
        Ann(val, typ) => mk_quoted_ctr(
            eval_ctr(quote, TermTag::Ann),
            vec![
                span_to_num(expr.span),
                codegen_all_expr(lhs, num, quote, val),
                codegen_all_expr(lhs, num, quote, typ),
            ],
        ),
        Sub(name, idx, rdx, expr) => mk_quoted_ctr(
            eval_ctr(quote, TermTag::Sub),
            vec![
                span_to_num(expr.span),
                mk_u60(name.encode()),
                mk_u60(*idx as u64),
                mk_u60(*rdx as u64),
                codegen_all_expr(lhs, num, quote, expr),
            ],
        ),
        Num(n) => mk_quoted_ctr(
            eval_ctr(quote, TermTag::Num),
            vec![span_to_num(expr.span), mk_u60(*n)],
        ),
        Binary(operator, left, right) => mk_quoted_ctr(
            eval_ctr(quote, TermTag::Binary),
            vec![
                mk_single_ctr(operator_to_constructor(*operator).to_owned()),
                span_to_num(expr.span),
                codegen_all_expr(lhs, num, quote, left),
                codegen_all_expr(lhs, num, quote, right),
            ],
        ),
        Hole(num) => mk_quoted_ctr(
            eval_ctr(quote, TermTag::Hole),
            vec![span_to_num(expr.span), mk_u60(*num)],
        ),
        Hlp(name) => mk_quoted_ctr(
            eval_ctr(quote, TermTag::Hlp),
            vec![span_to_num(expr.span), mk_u60(name.encode())],
        ),
        Str(input) => codegen_str(input),
        Err => panic!("Internal Error: Was not expecting an ERR node inside the HVM checker"),
    }
}

pub fn codegen_expr(quote: bool, expr: &Expr) -> Box<Term> {
    codegen_all_expr(false, &mut 0, quote, expr)
}

pub fn codegen_pattern(args: &mut usize, quote: bool, expr: &Expr) -> Box<Term> {
    codegen_all_expr(true, args, quote, expr)
}

pub fn codegen_type(args: &[desugared::Argument], typ: &desugared::Expr) -> Box<lang::Term> {
    if !args.is_empty() {
        let arg = &args[0];
        mk_quoted_ctr(
            eval_ctr(true, TermTag::All),
            vec![
                span_to_num(arg.span),
                mk_u60(arg.name.encode()),
                codegen_expr(true, &arg.tipo),
                lam(&arg.name, codegen_type(&args[1..], typ)),
            ],
        )
    } else {
        codegen_expr(true, typ)
    }
}

pub fn codegen_vec<T>(exprs: T) -> Box<Term>
where
    T: Iterator<Item = Box<Term>>,
{
    exprs.fold(mk_ctr("List.nil".to_string(), vec![]), |left, right| {
        mk_ctr("List.cons".to_string(), vec![right, left])
    })
}

pub fn codegen_rule_end(file: &mut lang::File, rule: &desugared::Rule) {
    let base_vars = lift_spine(
        (0..rule.pats.len())
            .map(|x| mk_var(&format!("x{}", x)))
            .collect::<Vec<Box<lang::Term>>>(),
    );

    file.rules.push(lang::Rule {
        lhs: mk_ctr(
            TermTag::HoasQ(rule.name.to_string()).to_string(),
            vec_preppend![
                mk_var("orig");
                base_vars
            ],
        ),
        rhs: mk_quoted_ctr(
            eval_ctr(false, TermTag::Fun(base_vars.len())),
            vec_preppend![
                mk_ctr_name(&rule.name),
                mk_var("orig");
                base_vars
            ],
        ),
    });

    file.rules.push(lang::Rule {
        lhs: mk_ctr(
            TermTag::HoasF(rule.name.to_string()).to_string(),
            vec_preppend![
                mk_var("orig");
                base_vars
            ],
        ),
        rhs: mk_quoted_ctr(
            eval_ctr(false, TermTag::Fun(base_vars.len())),
            vec_preppend![
                mk_ctr_name(&rule.name),
                mk_var("orig");
                base_vars
            ],
        ),
    });
}

pub fn codegen_rule(file: &mut lang::File, rule: &desugared::Rule) {
    let mut count = 0;

    let lhs_args = rule
        .pats
        .iter()
        .map(|x| codegen_pattern(&mut count, false, x))
        .collect::<Vec<Box<Term>>>();

    file.rules.push(lang::Rule {
        lhs: mk_ctr(
            TermTag::HoasQ(rule.name.to_string()).to_string(),
            vec_preppend![
                mk_var("orig");
                lhs_args
            ],
        ),
        rhs: codegen_expr(true, &rule.body),
    });

    if rule.name.data.0 == "HVM.log" {
        file.rules.push(lang::Rule {
            lhs: mk_ctr(
                TermTag::HoasF(rule.name.to_string()).to_string(),
                vec![
                    mk_var("orig"),
                    mk_var("a"),
                    mk_var("r"),
                    mk_var("log"),
                    mk_var("ret"),
                ],
            ),
            rhs: mk_ctr(
                "HVM.put".to_owned(),
                vec![
                    mk_ctr("HVM.Term.show".to_owned(), vec![mk_var("log")]),
                    mk_var("ret"),
                ],
            ),
        });
    } else {
        file.rules.push(lang::Rule {
            lhs: mk_ctr(
                TermTag::HoasF(rule.name.to_string()).to_string(),
                vec_preppend![
                    mk_var("orig");
                    lhs_args
                ],
            ),
            rhs: codegen_expr(false, &rule.body),
        });
    }
}

pub fn codegen_entry_rules(
    count: &mut usize,
    index: usize,
    args: &mut Vec<Box<Term>>,
    entry: &desugared::Rule,
    pats: &[Box<desugared::Expr>],
) -> Box<Term> {
    if pats.is_empty() {
        mk_ctr(
            "Kind.Rule.rhs".to_owned(),
            vec![mk_ctr(
                format!("QT{}", index),
                vec_preppend![
                    mk_ctr_name(&entry.name),
                    span_to_num(entry.span);
                    args
                ],
            )],
        )
    } else {
        let pat = &pats[0];
        let expr = codegen_pattern(count, false, pat);
        args.push(expr.clone());
        mk_ctr(
            "Kind.Rule.lhs".to_owned(),
            vec![
                expr,
                codegen_entry_rules(count, index + 1, args, entry, &pats[1..]),
            ],
        )
    }
}

pub fn codegen_entry(file: &mut lang::File, entry: &desugared::Entry) {
    file.rules.push(lang::Rule {
        lhs: mk_ctr("NameOf".to_owned(), vec![mk_ctr_name(&entry.name)]),
        rhs: codegen_str(entry.name.to_str()),
    });

    file.rules.push(lang::Rule {
        lhs: mk_ctr("HashOf".to_owned(), vec![mk_ctr_name(&entry.name)]),
        rhs: mk_u60(fxhash::hash64(entry.name.to_str())),
    });

    file.rules.push(lang::Rule {
        lhs: mk_ctr("TypeOf".to_owned(), vec![mk_ctr_name(&entry.name)]),
        rhs: codegen_type(&entry.args, &entry.tipo),
    });

    let base_vars = (0..entry.args.len())
        .map(|x| mk_var(&format!("x{}", x)))
        .collect::<Vec<Box<lang::Term>>>();

    file.rules.push(lang::Rule {
        lhs: mk_ctr(
            format!("Kind.Term.FN{}", entry.args.len()),
            vec_preppend![
                mk_ctr_name(&entry.name),
                mk_var("orig");
                lift_spine(base_vars.clone())
            ],
        ),
        rhs: mk_quoted_ctr(
            TermTag::HoasF(entry.name.to_string()).to_string(),
            vec_preppend![
                mk_var("orig");
                lift_spine(base_vars.clone())
            ],
        ),
    });

    file.rules.push(lang::Rule {
        lhs: mk_ctr(
            format!("QT{}", entry.args.len()),
            vec_preppend![
                mk_ctr_name(&entry.name),
                mk_var("orig");
                lift_spine(base_vars.clone())
            ],
        ),
        rhs: mk_quoted_ctr(
            TermTag::HoasQ(entry.name.to_string()).to_string(),
            vec_preppend![
                mk_var("orig");
                lift_spine(base_vars)
            ],
        ),
    });

    for rule in &entry.rules {
        codegen_rule(file, rule);
    }

    if !entry.rules.is_empty() {
        codegen_rule_end(file, &entry.rules[0])
    }

    let rules = entry
        .rules
        .iter()
        .map(|rule| codegen_entry_rules(&mut 0, 0, &mut Vec::new(), rule, &rule.pats));

    file.rules.push(lang::Rule {
        lhs: mk_ctr("RuleOf".to_owned(), vec![mk_ctr_name(&entry.name)]),
        rhs: codegen_vec(rules),
    });
}

pub fn codegen_glossary(glossary: &Glossary) -> lang::File {
    let mut file = lang::File { rules: vec![] };

    let functions_entry = lang::Rule {
        lhs: mk_ctr("Functions".to_owned(), vec![]),
        rhs: codegen_vec(glossary.entrs.values().map(|x| mk_ctr_name(&x.name))),
    };

    for entry in glossary.entrs.values() {
        codegen_entry(&mut file, entry)
    }

    file.rules.push(functions_entry);

    file
}
