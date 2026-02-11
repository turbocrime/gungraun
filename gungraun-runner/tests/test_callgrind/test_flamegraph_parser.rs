use gungraun_runner::api::EventKind;
use gungraun_runner::runner::callgrind::flamegraph_parser::FlamegraphParser;
use gungraun_runner::runner::callgrind::parser::{CallgrindParser, Sentinel};
use rstest::rstest;

use crate::common::{get_project_root, Fixtures};

#[rstest]
#[case::when_entry_point("when_entry_point", Some(Sentinel::new("benchmark_tests_exit::main")))]
#[case::no_entry_point("no_entry_point", None)]
#[case::branching("branching", Some(Sentinel::new("benchmark_tests_branching::main")))]
#[case::recursive("recursive", Some(Sentinel::new("benchmark_tests_recursive::main")))]
fn test_flamegraph_parser(#[case] name: &str, #[case] sentinel: Option<Sentinel>) {
    use gungraun_runner::api::ValgrindTool;
    use gungraun_runner::runner::tool::path::ToolOutputPathKind;

    let output = Fixtures::get_tool_output_path(
        "callgrind.out",
        ValgrindTool::Callgrind,
        ToolOutputPathKind::Out,
        name,
    );
    let expected_stacks =
        Fixtures::load_stacks(format!("callgrind.out/callgrind.{name}.exp_stacks"));
    let parser = FlamegraphParser::new(sentinel.as_ref(), get_project_root(), 0);

    let result = parser.parse(&output).unwrap();
    assert_eq!(result.len(), 1);
    let stacks = result[0].2.to_stack_format(&EventKind::Ir).unwrap();

    assert_eq!(stacks.len(), expected_stacks.len());
    // Assert line by line or else the output on error is unreadable. Also, provide an additional
    // line of context
    let mut failed = false;
    for (index, (stack, expected_stack)) in stacks.iter().zip(expected_stacks.iter()).enumerate() {
        if stack != expected_stack {
            if failed {
                print!(
                    "{}",
                    pretty_assertions::StrComparison::new(stack, expected_stack)
                );
                break;
            }
            failed = true;
            println!("Failed at index '{index}'");
            print!(
                "{}",
                pretty_assertions::StrComparison::new(stack, expected_stack)
            );
        }
    }

    assert!(!failed);
}

#[rstest]
#[case("branching", Some(Sentinel::new("benchmark_tests_branching::main")), 100)]
#[case("branching", Some(Sentinel::new("benchmark_tests_branching::main")), 1000)]
#[case("recursive", Some(Sentinel::new("benchmark_tests_recursive::main")), 100)]
#[case("recursive", Some(Sentinel::new("benchmark_tests_recursive::main")), 1000)]
fn test_flamegraph_parser_cost_culling(
    #[case] name: &str,
    #[case] sentinel: Option<Sentinel>,
    #[case] min_cost: u64,
) {
    use gungraun_runner::api::ValgrindTool;
    use gungraun_runner::runner::tool::path::ToolOutputPathKind;

    let output = Fixtures::get_tool_output_path(
        "callgrind.out",
        ValgrindTool::Callgrind,
        ToolOutputPathKind::Out,
        name,
    );
    let expected_stacks = Fixtures::load_stacks(format!(
        "callgrind.out/callgrind.{name}_cull{min_cost:04}.exp_stacks"
    ));
    let parser = FlamegraphParser::new(sentinel.as_ref(), get_project_root(), min_cost);
    let result = parser.parse(&output).unwrap();
    let stacks = result[0].2.to_stack_format(&EventKind::Ir).unwrap();

    assert_eq!(stacks.len(), expected_stacks.len());
    for (i, (got, want)) in stacks.iter().zip(expected_stacks.iter()).enumerate() {
        assert_eq!(got, want, "mismatch at index {i}");
    }
}
