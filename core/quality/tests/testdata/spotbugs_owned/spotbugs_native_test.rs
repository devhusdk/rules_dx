//! SpotBugs reports missing classes without the dependency on the aux classpath and succeeds with it.

use dx_testing::{env_file, env_files, path_text, run};
use std::path::PathBuf;

fn class_jar(var: &str) -> PathBuf {
    env_files(var)
        .into_iter()
        .find(|p| {
            let name = path_text(p);
            name.ends_with(".jar") && !name.contains("-hjar") && !name.contains("header") && !name.ends_with("-src.jar")
        })
        .unwrap_or_else(|| panic!("{var} has a class jar"))
}

fn sarif(args: &[&str]) -> String {
    let out = run(&env_file("SPOTBUGS"), args, &[]).expect("run spotbugs");
    out.stdout.chars().filter(|c| *c != ' ' && *c != '\n' && *c != '\r').collect()
}

#[test]
fn aux_classpath_completes_the_analysis() {
    let app = path_text(&class_jar("APP"));
    let mut aux = vec![class_jar("DEP")];
    aux.extend(env_files("JACOCO"));
    let aux = path_text(&PathBuf::from(std::env::join_paths(aux).expect("join aux classpath")));
    let missing = sarif(&["-textui", "-effort:default", "-sarif", &app]);
    let complete = sarif(&["-textui", "-effort:default", "-sarif", "-auxclasspath", &aux, &app]);
    assert!(missing.contains("\"executionSuccessful\":false"), "missing dependency run not marked failed");
    assert!(missing.contains("Classesneededforanalysiswere"), "missing class notification absent");
    assert!(complete.contains("\"executionSuccessful\":true"), "complete classpath run not marked successful");
    assert!(!complete.contains("Classesneededforanalysiswere"), "complete classpath run reports missing classes");
}
