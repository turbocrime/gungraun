#[inline(never)]
fn parse_arg(arg: &str) -> i32 {
    arg.parse::<i32>().expect("Valid status code")
}

#[inline(never)]
fn recursive(recurse: i32, arg: &str) -> (i32, i32) {
    let mut sum = 0i32;
    for i in 0..200 {
        sum = sum.wrapping_add(i as i32);
    }
    if recurse > 0 {
        let (child_sum, code) = recursive(recurse - 1, arg);
        (sum.wrapping_add(child_sum), code)
    } else {
        let code = parse_arg(arg);
        (sum, code)
    }
}

#[inline(never)]
fn get_arg(n: usize) -> String {
    std::env::args().nth(n).expect("Exit status")
}

fn main() {
    let arg = get_arg(1);
    let s: &str = &arg;
    let (sum, code) = recursive(3, s);
    std::hint::black_box(sum);
    std::process::exit(code);
}
