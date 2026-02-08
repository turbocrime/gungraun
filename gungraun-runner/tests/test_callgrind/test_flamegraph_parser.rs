use gungraun_runner::api::EventKind;
use gungraun_runner::runner::callgrind::flamegraph_parser::FlamegraphParser;
use gungraun_runner::runner::callgrind::parser::{CallgrindParser, Sentinel};
use rstest::rstest;

use crate::common::{get_project_root, Fixtures};

#[rstest]
#[case::when_entry_point("when_entry_point", Some(Sentinel::new("benchmark_tests_exit::main")))]
#[case::no_entry_point("no_entry_point", None)]
#[case::branching("branching", None)]
#[case::recursive("recursive", None)]
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
    let parser = FlamegraphParser::new(sentinel.as_ref(), get_project_root());

    let result = parser.parse(&output).unwrap();
    assert_eq!(result.len(), 1);
    let stacks = result[0].2.to_stack_format(&EventKind::Ir).unwrap();

    assert_eq!(stacks.len(), expected_stacks.len());
    // Assert line by line or else the output on error is unreadable. Also, provide an additional
    // line of context
    assert_stacks_match(&stacks, &expected_stacks);
}

#[test]
fn test_flamegraph_parser_multithread() {
    use gungraun_runner::api::ValgrindTool;
    use gungraun_runner::runner::tool::path::ToolOutputPathKind;

    let output = Fixtures::get_tool_output_path(
        "callgrind.out",
        ValgrindTool::Callgrind,
        ToolOutputPathKind::Out,
        "multithread",
    );
    let parser = FlamegraphParser::new(None, get_project_root());
    let result = parser.parse(&output).unwrap();

    // Two separate .out files → two parsed maps
    assert_eq!(result.len(), 2);

    // Results are sorted by compare_target_ids (thread ascending), so t0 first, t1 second
    let (_, props_t0, map_t0) = &result[0];
    let (_, props_t1, map_t1) = &result[1];

    assert_eq!(props_t0.thread, Some(0));
    assert_eq!(props_t1.thread, Some(1));

    // Verify each thread's stacks independently
    let stacks_t0 = map_t0.to_stack_format(&EventKind::Ir).unwrap();
    let expected_t0 =
        Fixtures::load_stacks("callgrind.out/callgrind.multithread.t0.exp_stacks");
    assert_eq!(stacks_t0.len(), expected_t0.len());
    assert_stacks_match(&stacks_t0, &expected_t0);

    let stacks_t1 = map_t1.to_stack_format(&EventKind::Ir).unwrap();
    let expected_t1 =
        Fixtures::load_stacks("callgrind.out/callgrind.multithread.t1.exp_stacks");
    assert_eq!(stacks_t1.len(), expected_t1.len());
    assert_stacks_match(&stacks_t1, &expected_t1);
}

fn assert_stacks_match(stacks: &[String], expected_stacks: &[String]) {
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
