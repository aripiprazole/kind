use kind_driver::session::Session;
use kind_report::data::Diagnostic;
use kind_report::report::Report;
use kind_report::RenderConfig;

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use ntest::timeout;
use pretty_assertions::assert_eq;
use walkdir::{Error, WalkDir};

use kind_driver as driver;

fn golden_test(path: &Path, run: fn(&Path) -> String) {
    let result = run(path);

    let golden_path = path.with_extension("golden");
    if let Ok(to_check) = fs::read_to_string(golden_path.clone()) {
        assert_eq!(result, to_check, "Testing file '{}'", path.display());
    } else {
        let mut file = File::create(golden_path).unwrap();
        file.write_all(result.as_bytes()).unwrap();
    }
}

fn test_kind2(path: &Path, run: fn(&Path) -> String) -> Result<(), Error> {
    for entry in WalkDir::new(path).follow_links(true) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().map(|x| x == "kind2").unwrap_or(false) {
            golden_test(path, run);
        }
    }
    Ok(())
}

#[test]
#[timeout(30000)]
fn test_checker() -> Result<(), Error> {
    test_kind2(Path::new("./suite/checker"), |path| {
        let (rx, tx) = std::sync::mpsc::channel();
        let root = PathBuf::from("./suite/lib").canonicalize().unwrap();
        let mut session = Session::new(root, rx);

        let entrypoints = vec!["Main".to_string()];
        let check = driver::type_check_book(&mut session, &PathBuf::from(path), entrypoints, Some(1));

        let diagnostics = tx.try_iter().collect::<Vec<Box<dyn Diagnostic>>>();
        let render = RenderConfig::ascii(2);

        kind_report::check_if_colors_are_supported(true);

        match check {
            Ok(_) if diagnostics.is_empty() => "Ok!".to_string(),
            _ => {
                let mut res_string = String::new();

                for diag in diagnostics {
                    diag.render(&mut session, &render, &mut res_string).unwrap();
                }

                res_string
            }
        }
    })?;
    Ok(())
}

#[test]
#[timeout(15000)]
fn test_eval() -> Result<(), Error> {
    test_kind2(Path::new("./suite/eval"), |path| {
        let (rx, tx) = std::sync::mpsc::channel();
        let root = PathBuf::from("./suite/lib").canonicalize().unwrap();
        let mut session = Session::new(root, rx);

        let entrypoints = vec!["Main".to_string()];
        let check = driver::erase_book(&mut session, &PathBuf::from(path), entrypoints)
            .map(|x| driver::compile_book_to_hvm(x, false));

        let diagnostics = tx.try_iter().collect::<Vec<_>>();
        let render = RenderConfig::ascii(2);

        kind_report::check_if_colors_are_supported(true);

        match check {
            Ok(file) if diagnostics.is_empty() => {
                driver::execute_file(&file.to_string(), Some(1)).map_or_else(|e| e, |f| f)
            }
            _ => {
                let mut res_string = String::new();

                for diag in diagnostics {
                    diag.render(&mut session, &render, &mut res_string).unwrap();
                }

                res_string
            }
        }
    })?;
    Ok(())
}


#[test]
#[timeout(15000)]
fn test_kdl() -> Result<(), Error> {
    test_kind2(Path::new("./suite/kdl"), |path| {
        let (rx, tx) = std::sync::mpsc::channel();
        let root = PathBuf::from("./suite/lib").canonicalize().unwrap();
        let mut session = Session::new(root, rx);

        let entrypoints = vec!["Main".to_string()];
        let check = driver::compile_book_to_kdl(&PathBuf::from(path), &mut session, "", entrypoints);

        let diagnostics = tx.try_iter().collect::<Vec<_>>();
        let render = RenderConfig::ascii(2);

        kind_report::check_if_colors_are_supported(true);

        match check {
            Ok(file) if diagnostics.is_empty() => {
                file.to_string()
            },
            _ => {
                let mut res_string = String::new();

                for diag in diagnostics {
                    diag.render(&mut session, &render, &mut res_string).unwrap();
                }

                res_string
            }
        }
    })?;
    Ok(())
}
