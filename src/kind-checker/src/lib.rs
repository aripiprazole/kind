//! A type checker for the kind2 language. It has some utilities
//! to [compile kind2 code][compiler] into a version that the checker
//! can understand and [transform the answer back][report] into a
//! version that the Rust side can manipulate.

pub mod compiler;
mod errors;
pub mod report;

use std::sync::mpsc::Sender;

use hvm::Term;
use kind_report::data::Diagnostic;
use kind_tree::desugared::Book;

use crate::report::parse_report;

const CHECKER_HVM: &str = include_str!("checker.hvm");

/// Generates the checker in a string format that can be
/// parsed by HVM.
pub fn gen_checker(book: &Book, functions_to_check: Vec<String>) -> String {
    let base_check_code = compiler::codegen_book(book, functions_to_check);
    let mut check_code = CHECKER_HVM.to_string();
    check_code.push_str(&base_check_code.to_string());

    check_code
}

/// Type checks a dessugared book. It spawns an HVM instance in order
/// to run a compiled version of the book
pub fn type_check(book: &Book, tx: Sender<Box<dyn Diagnostic>>, functions_to_check: Vec<String>) -> bool {
    let check_code = gen_checker(book, functions_to_check);

    let mut runtime = hvm::Runtime::from_code(&check_code).unwrap();
    let main = runtime.alloc_code("Kind.API.check_all").unwrap();
    runtime.run_io(main);
    runtime.normalize(main);
    let term = runtime.readback(main);

    let errs = parse_report(&term)
        .expect(&format!("Internal Error: Cannot parse the report message from the type checker: {}", term));
    let succeeded = errs.is_empty();

    for err in errs {
        tx.send(Box::new(err)).unwrap()
    }

    succeeded
}

/// Runs the type checker but instead of running the check all function
/// we run the "eval_main" that runs the generated version that both HVM and
/// and the checker can understand.
pub fn eval_api(book: &Book) -> Box<Term> {
    let check_code = gen_checker(book, book.names.keys().cloned().collect());

    let mut runtime = hvm::Runtime::from_code(&check_code).unwrap();
    let main = runtime.alloc_code("Kind.API.eval_main").unwrap();

    runtime.run_io(main);
    runtime.normalize(main);
    runtime.readback(main)
}
