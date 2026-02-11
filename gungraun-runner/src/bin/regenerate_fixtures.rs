//! Regenerate .exp_stacks fixture files from .out callgrind files.
//!
//! Run after `scripts/generate_callgrind_fixtures.sh` to update expected stacks:
//!   cargo run -p gungraun-runner --bin regenerate_fixtures
use std::io::Write;
use std::path::{Path, PathBuf};

use gungraun_runner::api::EventKind;
use gungraun_runner::runner::callgrind::flamegraph_parser::FlamegraphParser;
use gungraun_runner::runner::callgrind::parser::{CallgrindParser, Sentinel};
use gungraun_runner::runner::summary::BaselineKind;
use gungraun_runner::runner::tool::path::ToolOutputPathKind;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/callgrind.out")
}

fn project_root() -> PathBuf {
    let meta = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .expect("Querying metadata of cargo workspace succeeds");
    meta.workspace_root.into_std_path_buf()
}

fn write_stacks(stacks: &[String], path: &Path) {
    let mut f = std::fs::File::create(path).unwrap();
    for s in stacks {
        writeln!(f, "{s}").unwrap();
    }
    println!("Wrote {} lines to {}", stacks.len(), path.display());
}

fn parse_and_write(
    name: &str,
    sentinel: Option<&Sentinel>,
    min_cost: u64,
    suffix: &str,
) {
    use gungraun_runner::api::ValgrindTool;

    let fixtures = fixtures_dir();
    let out_path = gungraun_runner::runner::tool::path::ToolOutputPath {
        kind: ToolOutputPathKind::Out,
        tool: ValgrindTool::Callgrind,
        baseline_kind: BaselineKind::Old,
        dir: fixtures.clone(),
        name: name.to_owned(),
        modifiers: vec![],
    };

    let parser = FlamegraphParser::new(sentinel, project_root(), min_cost);
    let result = parser.parse(&out_path).unwrap();
    let stacks = result[0].2.to_stack_format(&EventKind::Ir).unwrap();
    let path = fixtures.join(format!("callgrind.{name}{suffix}.exp_stacks"));
    write_stacks(&stacks, &path);
}

fn main() {
    let sentinel = Sentinel::new("benchmark_tests_exit::main");

    let branching_sentinel = Sentinel::new("benchmark_tests_branching::main");
    let recursive_sentinel = Sentinel::new("benchmark_tests_recursive::main");

    // Base fixtures
    parse_and_write("when_entry_point", Some(&sentinel), 0, "");
    parse_and_write("no_entry_point", None, 0, "");
    parse_and_write("branching", Some(&branching_sentinel), 0, "");
    parse_and_write("recursive", Some(&recursive_sentinel), 0, "");

    // Culled fixtures
    for (name, s, min_cost) in [
        ("branching", &branching_sentinel, 100),
        ("branching", &branching_sentinel, 1000),
        ("recursive", &recursive_sentinel, 100),
        ("recursive", &recursive_sentinel, 1000),
    ] {
        parse_and_write(name, Some(s), min_cost, &format!("_cull{min_cost:04}"));
    }
}
