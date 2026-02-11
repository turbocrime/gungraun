#[inline(never)]
fn parse_arg(arg: &str) -> i32 {
    arg.parse::<i32>().expect("Valid status code")
}

#[inline(never)]
fn branch_a(arg: &str) -> (i32, i32) {
    let v = parse_arg(arg);
    let mut sum = 0i32;
    for i in 0..1000 {
        sum = sum.wrapping_add(v.wrapping_add(i));
    }
    (sum, v)
}

#[inline(never)]
fn branch_b(arg: &str) -> (i32, i32) {
    let v = parse_arg(arg);
    let mut sum = 0i32;
    for i in 0..500 {
        sum = sum.wrapping_add(v.wrapping_add(i));
    }
    (sum, v)
}

#[inline(never)]
fn get_arg(n: usize) -> String {
    std::env::args().nth(n).expect("Exit status")
}

fn main() {
    let arg = get_arg(1);
    let s: &str = &arg;
    let (sum_a, code) = branch_a(s);
    let (sum_b, _) = branch_b(s);
    std::hint::black_box(sum_a + sum_b);
    std::process::exit(code);
}
