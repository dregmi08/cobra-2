#[link(name = "our_code")]
extern "C" {
    #[link_name = "\x01our_code_starts_here"]
    fn our_code_starts_here(input: u64) -> u64;
}

#[no_mangle]
pub extern "C" fn snek_error(errcode: i64) {
    match errcode {
        1 => eprintln!("invalid argument"),
        2 => eprintln!("overflow"),
        _ => eprintln!("an error occurred {}", errcode),
    }
    std::process::exit(1);
}

fn parse_input(input: &str) -> u64 {
    match input {
        "true" => 3,
        "false" => 1,
        _ => {
            if let Ok(n) = input.parse::<i64>() {
                (n << 1) as u64
            } else {
                eprintln!("Invalid input");
                std::process::exit(1)
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = if args.len() > 1 { &args[1] } else { "false" };
    let input_val = parse_input(input);
    
    let result = unsafe { our_code_starts_here(input_val) };
    
    let output = if result & 1 == 0 {
        format!("{}", (result as i64) >> 1)
    } else if result == 3 {
        "true".to_string()
    } else {
        "false".to_string()
    };
    
    println!("{}", output);
}
