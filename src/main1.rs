use std::env; 
use std::fs::File;
use std::io::prelude::*;
use sexp::*;
use sexp::Atom::*;
use std::mem;
use std::collections::HashMap;
use std::io::{self, Write};
use dynasmrt::{dynasm, DynasmApi, DynasmLabelApi, DynamicLabel};


/*#[link(name = "our_code")]
extern "C" {
    // The \x01 here is an undocumented feature of LLVM that ensures
    // it does not add an underscore in front of the name.
    // Courtesy of Max New (https://maxsnew.com/teaching/eecs-483-fa22/hw_adder_assignment.html)
    #[link_name = "\x01our_code_starts_here"]
    fn our_code_starts_here(input: u64) -> u64;
}*/

//#[export_name = "\x01snek_error"]
#[no_mangle]
pub extern "C" fn snek_error(errcode: i64) {
    eprintln!("an error ocurred {errcode}");
    std::process::exit(1);
}

/*pub extern "C" fn snek_error(errcode: i64) {
    // TODO: print error message according to writeup
    eprintln!("an error ocurred {errcode}");
    std::process::exit(1);
}*/

enum Op1 { Add1, 
    Sub1,
    IsNum, 
    IsBool, 
}

enum Op2 { Plus, Minus, Times, Equal, Greater, GreaterEqual, Less, LessEqual, }

enum Expr {
    Number(i64),
    Boolean(bool),
    Id(String),
    Let(Vec<(String, Expr)>, Box<Expr>),
    UnOp(Op1, Box<Expr>),
    BinOp(Op2, Box<Expr>, Box<Expr>), //done up to here
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Loop(Box<Expr>),
    Break(Box<Expr>),
    Set(String, Box<Expr>),
    Block(Vec<Expr>),
    Define(String, Box<Expr>),
}


//USED CLAUDE: to figure out how to write the syntax for iterating through the List returned by
//sexp::parse and parsing for the block case
//PROMPT: I'm not sure what the rust syntax for iterating through a vector is, could you provide
//some clue provided this pattern matching code? + code
fn parse_expr(s: &Sexp, define_cnt: &mut i64, flag: &String) -> Expr {
    match s {
        Sexp::Atom(Atom::I(n)) => Expr::Number(i64::try_from(*n).unwrap()),
        Sexp::Atom(Atom::S(n))  => {
            match n.as_str() {
                "true" => Expr::Boolean(true),
                "false" => Expr::Boolean(false),
                _  => Expr::Id(n.clone()),
            }
        }
        Sexp::List(vec) => {
            match &vec[..] {
                [Sexp::Atom(S(op)), e] if op == "isnum" => Expr::UnOp(Op1::IsNum, Box::new(parse_expr(e, define_cnt, flag))),
                [Sexp::Atom(S(op)), e] if op == "isbool" => Expr::UnOp(Op1::IsBool, Box::new(parse_expr(e, define_cnt, flag))),
                [Sexp::Atom(S(op)), e] if op == "add1" => Expr::UnOp(Op1::Add1, Box::new(parse_expr(e, define_cnt, flag))),
                [Sexp::Atom(S(op)), e] if op == "sub1" => Expr::UnOp(Op1::Sub1, Box::new(parse_expr(e, define_cnt, flag))),
                [Sexp::Atom(S(op)), e1, e2] if op == "+" => Expr::BinOp(Op2::Plus, Box::new(parse_expr(e1, define_cnt, flag)), 
                    Box::new(parse_expr(e2, define_cnt, flag))),
                [Sexp::Atom(S(op)), e1, e2] if op == "-" => Expr::BinOp(Op2::Minus, Box::new(parse_expr(e1, define_cnt, flag)), 
                    Box::new(parse_expr(e2, define_cnt, flag))),
                [Sexp::Atom(S(op)), e1, e2] if op == "*" => Expr::BinOp(Op2::Times, Box::new(parse_expr(e1, define_cnt, flag)), 
                    Box::new(parse_expr(e2, define_cnt, flag))),
                [Sexp::Atom(S(op)), e1, e2] if op == "=" => Expr::BinOp(Op2::Equal, Box::new(parse_expr(e1, define_cnt, flag)), 
                    Box::new(parse_expr(e2, define_cnt, flag))),
                [Sexp::Atom(S(op)), e1, e2] if op == ">" => Expr::BinOp(Op2::Greater, Box::new(parse_expr(e1, define_cnt, flag)),
                    Box::new(parse_expr(e2, define_cnt, flag))),
                [Sexp::Atom(S(op)), e1, e2] if op == ">=" => Expr::BinOp(Op2::GreaterEqual, 
                    Box::new(parse_expr(e1, define_cnt, flag)),
                    Box::new(parse_expr(e2, define_cnt, flag))),
                [Sexp::Atom(S(op)), e1, e2] if op == "<" => Expr::BinOp(Op2::Less,
                    Box::new(parse_expr(e1, define_cnt, flag)),
                    Box::new(parse_expr(e2, define_cnt, flag))),
                [Sexp::Atom(S(op)), e1, e2] if op == "<=" => Expr::BinOp(Op2::LessEqual,
                    Box::new(parse_expr(e1, define_cnt, flag)),
                    Box::new(parse_expr(e2, define_cnt, flag))),
                [Sexp::Atom(S(op)), cond, true_con, false_con] if op == "if" => Expr::If(
                    Box::new(parse_expr(cond, define_cnt, flag)),
                    Box::new(parse_expr(true_con, define_cnt, flag)),
                    Box::new(parse_expr(false_con, define_cnt, flag))),
                [Sexp::Atom(S(op)), code] if op == "loop" => Expr::Loop(Box::new(parse_expr(code, define_cnt, flag))),
                [Sexp::Atom(S(op)), code] if op == "break" => Expr::Break(Box::new(parse_expr(code, define_cnt, flag))),
                [Sexp::Atom(S(op)), Sexp::Atom(S(var_name)), e] if op == "set!" => Expr::Set(var_name.clone(), 
                    Box::new(parse_expr(e, define_cnt, flag))),
                [Sexp::Atom(S(op)), exprs @ ..] if op == "block" => {
                    if exprs.is_empty() {
                        panic!("Invalid: parse error");
                    }
                    let block_exprs = exprs.iter()
                        .map(|e| parse_expr(e, define_cnt, flag))
                        .collect();
                    Expr::Block(block_exprs)
                }
                [Sexp::Atom(S(op)), Sexp::Atom(Atom::S(n)), e] if op == "define" => {
                    *define_cnt += 1;
                    if *define_cnt > 1 {
                        panic!("Invalid: parse error");
                    }
                    if flag != "-i" {
                        panic!("Invalid: parse error");
                    }
                    Expr::Define(n.clone(),Box::new(parse_expr(e, define_cnt, flag)))
                }
                [Sexp::Atom(S(op)), Sexp::List(bindings), e] if op == "let" => {
                    let bindings = parse_bind(bindings, define_cnt, flag);
                    Expr::Let(bindings, Box::new(parse_expr(e, define_cnt, flag)))
                }
                _ => panic!("Invalid: parse error"),
            }
        },
        _ => panic!("Invalid: parse error"),
    }
}

fn parse_bind(bindings: &Vec<Sexp>, define_cnt: &mut i64, flag: &String) -> Vec<(String, Expr)> {
    let mut seen_names = std::collections::HashSet::new();
    
    bindings.iter().map(|binding| {
        match binding {
            Sexp::List(pair) => match &pair[..] {
                [Sexp::Atom(Atom::S(var_name)), exp] => {
                    if !seen_names.insert(var_name.clone()) {
                        panic!("Duplicate binding");
                    }
                    (var_name.clone(), parse_expr(exp, define_cnt, flag))
                }
                _ => panic!("Invalid binding pair: {:?}", pair),
            },
            _ => panic!("Each binding should be a list"),
        }
    }).collect()
}


//CLAUDE to help refactor repl to look much cleaner
//Prompt: Given this main code, could you refactor to look cleaner?
fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    let flag = &args[1];
    let mut def_cnt: i64 = 0;
    match flag.as_str() {
        "-i" => {
            if args.len() != 2 {
                eprintln!("Error: -i requires no args");
                std::process::exit(1);
            }
            repl(flag);
        }
        "-c" => {
            let in_name = &args[2];

            let mut in_file = File::open(in_name)?;
            let mut in_contents = String::new();
            in_file.read_to_string(&mut in_contents)?;
            let expr = parse_expr(&parse(&in_contents).unwrap(), &mut def_cnt, flag);
            // Compile to assembly only
            if args.len() < 4 {
                eprintln!("Error: -c requires output file");
                std::process::exit(1);
            }
            let out_name = &args[3];
            let env = HashMap::new();
            let define_env = HashMap::new();
            let result = compile_expr(&expr, 2, &env, &define_env, &None);
            let asm_program = format!("
        section .text
        global our_code_starts_here
        our_code_starts_here:
        {}
        ret
        ", result);
            let mut out_file = File::create(out_name)?;
            out_file.write_all(asm_program.as_bytes())?;
        }
        "-e" => {
            let in_name = &args[2];
    
            let mut in_file = File::open(in_name)?;
            let mut in_contents = String::new();
            in_file.read_to_string(&mut in_contents)?;
            let expr = parse_expr(&parse(&in_contents).unwrap(), &mut def_cnt, flag);
            
            // Parse the optional input argument (default to false if not provided)
            let input_val = if args.len() > 3 {
                parse_input(&args[3])
            } else {
                1  // false = 1 (tagged boolean)
            };
            
            let mut ops = dynasmrt::x64::Assembler::new().unwrap();
            let start = ops.offset();
            
            // Set up rdi with the input value before calling compiled code
            dynasm!(ops ; .arch x64 ; mov rdi, QWORD input_val);
            
            let env_ops = HashMap::new();
            let define_env = HashMap::new();
            compile_ops(&expr, &mut ops, 2, &env_ops, &define_env, None);
            dynasm!(ops ; .arch x64 ; ret);
            let buf = ops.finalize().unwrap();
            let jitted_fn: extern "C" fn() -> i64 = unsafe { mem::transmute(buf.ptr(start)) };
            let result = jitted_fn();
            
            // Decode the result based on tag bit
            let output = if result & 1 == 0 {
                // It's a number - shift right to get actual value
                format!("{}", result >> 1)
            } else {
                // It's a boolean
                if result == 3 {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            };
            println!("{}", output);
        }
        "-g" => {
            let in_name = &args[2];

            let mut in_file = File::open(in_name)?;
            let mut in_contents = String::new();
            in_file.read_to_string(&mut in_contents)?;
            let expr = parse_expr(&parse(&in_contents).unwrap(), &mut def_cnt, flag);

            if args.len() < 4 {
                eprintln!("Error: -g requires output file");
                std::process::exit(1);
            }
            let out_name = &args[3];
            
            // Parse the optional input argument (default to false if not provided)
            let input_val = if args.len() > 4 {
                parse_input(&args[4])
            } else {
                1  // false = 1 (tagged boolean)
            };
            
            // Write assembly
            let env = HashMap::new();
            let define_env = HashMap::new();
            let result = compile_expr(&expr, 2, &env, &define_env, &None);
            let asm_program = format!("
        section .text
        global our_code_starts_here
        our_code_starts_here:
        {}
        ret
        ", result);
            let mut out_file = File::create(out_name)?;
            out_file.write_all(asm_program.as_bytes())?;
            
            // JIT compile and run with input
            let mut ops = dynasmrt::x64::Assembler::new().unwrap();
            let start = ops.offset();
            
            // Set up rdi with the input value before calling compiled code
            dynasm!(ops ; .arch x64 ; mov rdi, QWORD input_val);
            
            let env_ops = HashMap::new();
            let define_env = HashMap::new();
            compile_ops(&expr, &mut ops, 2, &env_ops, &define_env, None);
            dynasm!(ops ; .arch x64 ; ret);
            let buf = ops.finalize().unwrap();
            let jitted_fn: extern "C" fn() -> i64 = unsafe { mem::transmute(buf.ptr(start)) };
            let result = jitted_fn();
            
            // Decode the result based on tag bit
            let output = if result & 1 == 0 {
                // It's a number - shift right to get actual value
                format!("{}", result >> 1)
            } else {
                // It's a boolean
                if result == 3 {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            };
            println!("{}", output);
        }
        _ => {
            eprintln!("Unknown flag: {}. Use -c, -e, -g, or -i", flag);
            std::process::exit(1);
        }
    }
    
    Ok(())
}

// Helper function to parse input arguments
fn parse_input(input: &str) -> i64 {
    match input {
        "true" => 3,   // true tagged as 3
        "false" => 1,  // false tagged as 1
        _ => {
            // Try to parse as a number
            match input.parse::<i64>() {
                Ok(n) => {
                    // Check for 63-bit overflow
                    if n < -4611686018427387904 || n > 4611686018427387903 {
                        eprintln!("Invalid: number overflow");
                        std::process::exit(1);
                    }
                    n << 1  // Tag as number (shift left by 1)
                }
                Err(_) => {
                    eprintln!("Invalid input: must be 'true', 'false', or a number");
                    std::process::exit(1);
                }
            }
        }
    }
}

//Claude usage: I used Claude to correct assembly at certain points: periodically
//Prompts were comething like this: My code doesn't work in this particular case {test}, could you
//advise me as to where I went wrong

fn compile_expr(e: &Expr, si: i32, env: &HashMap<String, i32>, define_env: &HashMap<String, i64>, break_target: &Option<String>) -> String {
    match e {
        Expr::Number(n) => {
            // Check for overflow (63-bit signed range)
            if *n < -4611686018427387904 || *n > 4611686018427387903 {
                panic!("Invalid: number overflow");
            }
            // Tag as number (shift left by 1, last bit = 0)
            format!("mov rax, {}", *n << 1)
        }
        Expr::Boolean(b) => {
            // true = 3 (0b11), false = 1 (0b01)
            let val = if *b { 3 } else { 1 };
            format!("mov rax, {}", val)
        }
        Expr::Id(name) => {
            if name == "input" {
                format!("mov rax, rdi")
            } else {
                let stack_offset = env.get(name).expect(&format!("Unbound variable identifier {}", name));
                format!("mov rax, [rsp - {}]", stack_offset)
            }
        }
        Expr::UnOp(Op1::Add1, subexpr) => {
            let e_instrs = compile_expr(subexpr, si, env, define_env, break_target);
            format!("{}\ntest rax, 1\njnz snek_error\nadd rax, 2", e_instrs)
        }
        Expr::UnOp(Op1::Sub1, subexpr) => {
            let e_instrs = compile_expr(subexpr, si, env, define_env, break_target);
            format!("{}\ntest rax, 1\njnz snek_error\nsub rax, 2", e_instrs)
        }
        Expr::UnOp(Op1::IsNum, subexpr) => {
            let e_instrs = compile_expr(subexpr, si, env, define_env, break_target);
            format!("{}\ntest rax, 1\nmov rbx, 1\nmov rcx, 3\ncmovz rax, rcx\ncmovnz rax, rbx", e_instrs)
        }
        Expr::UnOp(Op1::IsBool, subexpr) => {
            let e_instrs = compile_expr(subexpr, si, env, define_env, break_target);
            format!("{}\ntest rax, 1\nmov rbx, 1\nmov rcx, 3\ncmovnz rax, rcx\ncmovz rax, rbx", e_instrs)
        }
        Expr::BinOp(Op2::Plus, e1, e2) => {
            let expr1_instrs = compile_expr(e1, si, env, define_env, break_target);
            let expr2_instrs = compile_expr(e2, si+1, env, define_env, break_target);
            let stack_offset = si*8;
            format!("
               {expr1_instrs}
               test rax, 1
               jnz snek_error
               mov [rsp - {stack_offset}], rax
               {expr2_instrs}
               test rax, 1
               jnz snek_error
               add rax, [rsp - {stack_offset}]
               jo snek_error
            ")
        }
        Expr::BinOp(Op2::Minus, e1, e2) => {
            let expr1_instrs = compile_expr(e1, si, env, define_env, break_target);
            let expr2_instrs = compile_expr(e2, si+1, env, define_env, break_target);
            let stack_offset = si*8;
            format!("
               {expr1_instrs}
               test rax, 1
               jnz snek_error
               mov [rsp - {stack_offset}], rax
               {expr2_instrs}
               test rax, 1
               jnz snek_error
               mov rbx, rax
               mov rax, [rsp - {stack_offset}]
               sub rax, rbx
               jo snek_error
            ")
        }
        Expr::BinOp(Op2::Times, e1, e2) => {
            let expr1_instrs = compile_expr(e1, si, env, define_env, break_target);
            let expr2_instrs = compile_expr(e2, si+1, env, define_env, break_target);
            let stack_offset = si*8;
            format!("
               {expr1_instrs}
               test rax, 1
               jnz snek_error
               mov [rsp - {stack_offset}], rax
               {expr2_instrs}
               test rax, 1
               jnz snek_error
               sar rax, 1
               imul rax, [rsp - {stack_offset}]
               jo snek_error
            ")
        }
        Expr::BinOp(Op2::Equal, e1, e2) => {
            let expr1_instrs = compile_expr(e1, si, env, define_env, break_target);
            let expr2_instrs = compile_expr(e2, si+1, env, define_env, break_target);
            let stack_offset = si*8;
            format!("
               {expr1_instrs}
               mov [rsp - {stack_offset}], rax
               {expr2_instrs}
               mov rbx, rax
               xor rbx, [rsp - {stack_offset}]
               test rbx, 1
               jnz snek_error
               cmp rax, [rsp - {stack_offset}]
               mov rax, 1
               mov rbx, 3
               cmove rax, rbx
            ")
        }
        Expr::BinOp(Op2::Greater, e1, e2) => {
            let expr1_instrs = compile_expr(e1, si, env, define_env, break_target);
            let expr2_instrs = compile_expr(e2, si+1, env, define_env, break_target);
            let stack_offset = si*8;
            format!("
               {expr1_instrs}
               test rax, 1
               jnz snek_error
               mov [rsp - {stack_offset}], rax
               {expr2_instrs}
               test rax, 1
               jnz snek_error
               mov rbx, rax
               mov rax, [rsp - {stack_offset}]
               cmp rax, rbx
               mov rax, 1
               mov rbx, 3
               cmovg rax, rbx
            ")
        }
        Expr::BinOp(Op2::GreaterEqual, e1, e2) => {
            let expr1_instrs = compile_expr(e1, si, env, define_env, break_target);
            let expr2_instrs = compile_expr(e2, si+1, env, define_env, break_target);
            let stack_offset = si*8;
            format!("
               {expr1_instrs}
               test rax, 1
               jnz snek_error
               mov [rsp - {stack_offset}], rax
               {expr2_instrs}
               test rax, 1
               jnz snek_error
               mov rbx, rax
               mov rax, [rsp - {stack_offset}]
               cmp rax, rbx
               mov rax, 1
               mov rbx, 3
               cmovge rax, rbx
            ")
        }
        Expr::BinOp(Op2::Less, e1, e2) => {
            let expr1_instrs = compile_expr(e1, si, env, define_env, break_target);
            let expr2_instrs = compile_expr(e2, si+1, env, define_env, break_target);
            let stack_offset = si*8;
            format!("
               {expr1_instrs}
               test rax, 1
               jnz snek_error
               mov [rsp - {stack_offset}], rax
               {expr2_instrs}
               test rax, 1
               jnz snek_error
               mov rbx, rax
               mov rax, [rsp - {stack_offset}]
               cmp rax, rbx
               mov rax, 1
               mov rbx, 3
               cmovl rax, rbx
            ")
        }
        Expr::BinOp(Op2::LessEqual, e1, e2) => {
            let expr1_instrs = compile_expr(e1, si, env, define_env, break_target);
            let expr2_instrs = compile_expr(e2, si+1, env, define_env, break_target);
            let stack_offset = si*8;
            format!("
               {expr1_instrs}
               test rax, 1
               jnz snek_error
               mov [rsp - {stack_offset}], rax
               {expr2_instrs}
               test rax, 1
               jnz snek_error
               mov rbx, rax
               mov rax, [rsp - {stack_offset}]
               cmp rax, rbx
               mov rax, 1
               mov rbx, 3
               cmovle rax, rbx
            ")
        }
        Expr::If(cond, thn, els) => {
            static mut IF_COUNTER: i32 = 0;
            let label_num = unsafe {
                IF_COUNTER += 1;
                IF_COUNTER
            };
            let else_label = format!("if_else_{}", label_num);
            let end_label = format!("if_end_{}", label_num);
            
            let cond_instrs = compile_expr(cond, si, env, define_env, break_target);
            let thn_instrs = compile_expr(thn, si, env, define_env, break_target);
            let els_instrs = compile_expr(els, si, env, define_env, break_target);
            
            format!("
               {}
               cmp rax, 1
               je {}
               {}
               jmp {}
               {}:
               {}
               {}:
            ", cond_instrs, else_label, thn_instrs, end_label, else_label, els_instrs, end_label)
        }
        Expr::Loop(body) => {
            static mut LOOP_COUNTER: i32 = 0;
            let label_num = unsafe {
                LOOP_COUNTER += 1;
                LOOP_COUNTER
            };
            let start_label = format!("loop_start_{}", label_num);
            let end_label = format!("loop_end_{}", label_num);
            
            let body_instrs = compile_expr(body, si, env, define_env, &Some(end_label.clone()));
            
            format!("
               {}:
               {}
               jmp {}
               {}:
            ", start_label, body_instrs, start_label, end_label)
        }
        Expr::Break(e) => {
            match break_target {
                Some(label) => {
                    let e_instrs = compile_expr(e, si, env, define_env, break_target);
                    format!("{}\njmp {}", e_instrs, label)
                }
                None => panic!("Invalid: break outside of loop")
            }
        }
        Expr::Set(name, e) => {
            if !env.contains_key(name) {
                panic!("Unbound variable identifier {}", name);
            }
            let e_instrs = compile_expr(e, si, env, define_env, break_target);
            let stack_offset = env.get(name).unwrap();
            format!("{}\nmov [rsp - {}], rax", e_instrs, stack_offset)
        }
        Expr::Block(exprs) => {
            exprs.iter()
                .map(|e| compile_expr(e, si, env, define_env, break_target))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Expr::Let(bindings_vec, body) => {
            let mut new_env = env.clone();
            let mut instrs = String::new();
            let mut current_si = si;

            for (var_name, binding_expr) in bindings_vec {
                // Check for reserved keywords
                if is_keyword(var_name) {
                    panic!("Invalid: keyword used as identifier");
                }
                
                let binding_instrs = compile_expr(binding_expr, current_si, &new_env, define_env, break_target);
                let stack_offset = current_si * 8;
                instrs.push_str(&format!("
                    {}
                    mov [rsp - {}], rax
                ", binding_instrs, stack_offset));
                new_env.insert(var_name.clone(), stack_offset);
                current_si += 1;
            }

            let body_instrs = compile_expr(body, current_si, &new_env, define_env, break_target);
            instrs.push_str(&body_instrs);
            instrs
        }
        Expr::Define(var, e) => {
            compile_expr(e, si, env, define_env, break_target)
        }
    }
}



// Track define variables - either nown (immediate) or Unknown (pointer to heap)
#[derive(Clone)]
enum DefineValue {
    Known(i64),           // Direct immediate value
    Unknown(*mut i64),    // Pointer to heap-allocated value
}

// Helper function to check if a variable is ever set! in an expression
//Used CLAUDE to generate helper to see if a var's value is changed in a set!
//PROMPT: Given my compile_ops + compile_expr code, could you please help me generate a function
//that would help me determine where a var is changed in a set!?
fn is_set_in_expr(var_name: &str, expr: &Expr) -> bool {
    match expr {
        Expr::Set(name, e) => name == var_name || is_set_in_expr(var_name, e),
        Expr::UnOp(_, e) => is_set_in_expr(var_name, e),
        Expr::BinOp(_, e1, e2) => is_set_in_expr(var_name, e1) || is_set_in_expr(var_name, e2),
        Expr::If(cond, thn, els) => {
            is_set_in_expr(var_name, cond) || 
            is_set_in_expr(var_name, thn) || 
            is_set_in_expr(var_name, els)
        }
        Expr::Let(bindings, body) => {
            bindings.iter().any(|(_, e)| is_set_in_expr(var_name, e)) ||
            is_set_in_expr(var_name, body)
        }
        Expr::Loop(body) => is_set_in_expr(var_name, body),
        Expr::Break(e) => is_set_in_expr(var_name, e),
        Expr::Block(exprs) => exprs.iter().any(|e| is_set_in_expr(var_name, e)),
        Expr::Define(_, e) => is_set_in_expr(var_name, e),
        _ => false,
    }
}

//Used CLAUDE to help refactor repl to look much cleaner
//Prompt: Given this REPL code, could you refactor to look cleaner?
fn repl(flag: &String) -> io::Result<()> {
    let mut define_env: HashMap<String, DefineValue> = HashMap::new();

    loop {
        let mut define_count = 0;
        let mut buffer = String::new();
        io::stdout().write_all(b">")?;
        io::stdout().flush()?;
        io::stdin().read_line(&mut buffer)?;

        let trimmed_string = buffer.trim_end();
        if trimmed_string.is_empty() {
           continue;
        }
        if trimmed_string == "quit" || trimmed_string == "exit" {
            break;
        }

        let parsed = match parse(trimmed_string) {
            Ok(p) => p,
            Err(e) => {
                println!("{}", e);
                continue;
            }
        };

        let expr = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            parse_expr(&parsed, &mut define_count, flag)
        })) {
            Ok(e) => e,
            Err(e) => {
                let msg = e.downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "Unknown error".to_string());
                println!("{}", msg);
                continue;
            }
        };

        // Handle simple variable lookups
        match &expr {
            Expr::Id(var) => {
                if let Some(def_val) = define_env.get(var) {
                    let result = match def_val {
                        DefineValue::Known(val) => *val,
                        DefineValue::Unknown(ptr) => unsafe { **ptr },
                    };
                    let output = if result & 1 == 0 {
                        format!("{}", result >> 1)
                    } else {
                        if result == 3 { "true".to_string() } else { "false".to_string() }
                    };
                    println!("{}", output);
                    continue;
                } else {
                    println!("Unbound variable identifier {}", var);
                    continue;
                }
            }
            _ => {}
        }

        // Check if this is a define statement and if the variable is set!
        let (is_define, var_name, will_be_set) = match &expr {
            Expr::Define(name, body) => (true, name.clone(), is_set_in_expr(name, body)),
            _ => (false, String::new(), false),
        };

        // If defining a variable that will be set!, allocate on heap
        if is_define && will_be_set {
            // Allocate a box for this variable
            let boxed = Box::new(0i64); // Initial value, will be updated after compilation
            let ptr = Box::into_raw(boxed);
            define_env.insert(var_name.clone(), DefineValue::Unknown(ptr));
        }

        // Create a fresh assembler for each expression
        let mut ops = dynasmrt::x64::Assembler::new().unwrap();
        let start = ops.offset();
        let env = HashMap::new();
        
        // Catch panics from compile_ops
        let compile_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compile_ops(&expr, &mut ops, 2, &env, &define_env, None);
            dynasm!(ops ; .arch x64 ; ret);
            ops.commit().unwrap();
            ops
        }));
        
        let ops = match compile_result {
            Ok(assembled_ops) => assembled_ops,
            Err(e) => {
                let msg = e.downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "Unknown error".to_string());
                println!("{}", msg);
                continue;
            }
        };
        
        let reader = ops.reader();
        let buf = reader.lock();
        let jitted_fn: extern "C" fn() -> i64 = unsafe { mem::transmute(buf.ptr(start)) };
        let result = jitted_fn();
       
        // Update define environment based on result
        match expr {
            Expr::Define(var, _) => {
                if let Some(DefineValue::Unknown(ptr)) = define_env.get(&var) {
                    // Update the heap value
                    unsafe { **ptr = result; }
                } else {
                    // Store as known immediate
                    define_env.insert(var, DefineValue::Known(result));
                }
                continue;
            }
            _ => {}
        }

        // Output result
        let output = if result & 1 == 0 {
            format!("{}", result >> 1)
        } else {
            if result == 3 { "true".to_string() } else { "false".to_string() }
        };
        println!("{}", output);
    }
    Ok(())
}


//CLAUDE USAGE: Used Claude to reformat function to check if types in binop are the same
//Prmopt: Given the function, reformat function to be cleaner + reformat/correct rust syntax
fn get_static_type(e: &Expr, env: &HashMap<String, i32>, define_env: &HashMap<String, DefineValue>) -> Option<bool> {
    // Returns Some(false) for numbers, Some(true) for bools, None if unknown at compile time
    match e {
        Expr::Number(_) => Some(false), // It's a number
        Expr::Boolean(_) => Some(true), // It's a bool
        Expr::Id(name) => {
            if name == "input" {
                None // Unknown until runtime
            } else if let Some(DefineValue::Known(val)) = define_env.get(name) {
                // Check the tag bit
                Some(val & 1 == 1)
            } else {
                None // Stack var or unknown define var
            }
        }
        _ => None // Complex expressions - unknown at compile time
    }
}

//Claude usage: I used Claude to correct assembly at certain points: periodically
//Prompts were comething like this: My code doesn't work in this particular case {test}, could you
//advise me as to where I went wrong
fn compile_ops(
    e: &Expr,
    ops: &mut dynasmrt::x64::Assembler,
    si: i32,
    env: &HashMap<String, i32>,
    define_env: &HashMap<String, DefineValue>,
    break_label: Option<dynasmrt::DynamicLabel>
) {
    match e {
        Expr::Number(n) => {
            let tagged = *n << 1;
            dynasm!(ops ; .arch x64 ; mov rax, QWORD tagged as i64);
        }
        Expr::Boolean(b) => {
            let val = if *b { 3 } else { 1 };
            dynasm!(ops ; .arch x64 ; mov rax, QWORD val);
        }
        Expr::Id(name) => {
            if name == "input" {
                dynasm!(ops ; .arch x64 ; mov rax, rdi);
            } else if let Some(stack_offset) = env.get(name) {
                dynasm!(ops ; .arch x64 ; mov rax, [rsp - *stack_offset]);
            } else if let Some(def_val) = define_env.get(name) {
                match def_val {
                    DefineValue::Known(val) => {
                        dynasm!(ops ; .arch x64 ; mov rax, QWORD *val);
                    }
                    DefineValue::Unknown(ptr) => {
                        let ptr_val = *ptr as i64;
                        dynasm!(ops ; .arch x64
                            ; mov rax, QWORD ptr_val
                            ; mov rax, [rax]
                        );
                    }
                }
            } else {
                panic!("Unbound variable identifier {}", name);
            }
        }
        Expr::UnOp(Op1::Add1, e1) => {
            // Check if we know statically that e1 is not a number
            if let Some(true) = get_static_type(e1, env, define_env) {
                panic!("Invalid: invalid argument");
            }

            compile_ops(e1, ops, si, env, define_env, break_label);
            let error_label = ops.new_dynamic_label();
            let ok_label = ops.new_dynamic_label();
            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; add rax, 2
                ; jmp =>ok_label
                ; =>error_label
                ; mov rdi, 1
                ; mov rax, 60
                ; syscall
                ; =>ok_label
            );
        }
        Expr::UnOp(Op1::Sub1, e1) => {
            // Check if we know statically that e1 is not a number
            if let Some(true) = get_static_type(e1, env, define_env) {
                panic!("Invalid: invalid argument");
            }

            compile_ops(e1, ops, si, env, define_env, break_label);
            let error_label = ops.new_dynamic_label();
            let ok_label = ops.new_dynamic_label();
            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; sub rax, 2
                ; jmp =>ok_label
                ; =>error_label
                ; mov rdi, 1
                ; mov rax, 60
                ; syscall
                ; =>ok_label
            );
        }
        Expr::UnOp(Op1::IsNum, e1) => {
            compile_ops(e1, ops, si, env, define_env, break_label);
            let stack_offset = si * 8;
            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; mov rax, 1
                ; mov QWORD [rsp - stack_offset], 3
                ; cmovz rax, [rsp - stack_offset]
            );
        }
        Expr::UnOp(Op1::IsBool, e1) => {
            compile_ops(e1, ops, si, env, define_env, break_label);
            let stack_offset = si * 8;
            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; mov rax, 1
                ; mov QWORD [rsp - stack_offset], 3
                ; cmovnz rax, [rsp - stack_offset]
            );
        }
        Expr::BinOp(Op2::Plus, e1, e2) => {
            // Check if we know statically that either operand is not a number
            if let Some(true) = get_static_type(e1, env, define_env) {
                panic!("Invalid: invalid argument");
            }
            if let Some(true) = get_static_type(e2, env, define_env) {
                panic!("Invalid: invalid argument");
            }

            compile_ops(e1, ops, si, env, define_env, break_label);
            let stack_offset = si * 8;
            let error_label = ops.new_dynamic_label();

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset], rax
            );

            compile_ops(e2, ops, si+1, env, define_env, break_label);

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; add rax, [rsp - stack_offset]
            );

            let ok_label = ops.new_dynamic_label();
            dynasm!(ops ; .arch x64
                ; jmp =>ok_label
                ; =>error_label
                ; mov rdi, 1
                ; mov rax, 60
                ; syscall
                ; =>ok_label
            );
        }
        Expr::BinOp(Op2::Minus, e1, e2) => {
            // Check if we know statically that either operand is not a number
            if let Some(true) = get_static_type(e1, env, define_env) {
                panic!("Invalid: invalid argument");
            }
            if let Some(true) = get_static_type(e2, env, define_env) {
                panic!("Invalid: invalid argument");
            }

            compile_ops(e1, ops, si, env, define_env, break_label);
            let stack_offset = si * 8;
            let stack_offset2 = (si + 1) * 8;
            let error_label = ops.new_dynamic_label();

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset], rax
            );

            compile_ops(e2, ops, si+1, env, define_env, break_label);

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset2], rax
                ; mov rax, [rsp - stack_offset]
                ; sub rax, [rsp - stack_offset2]
            );

            let ok_label = ops.new_dynamic_label();
            dynasm!(ops ; .arch x64
                ; jmp =>ok_label
                ; =>error_label
                ; mov rdi, 1
                ; mov rax, 60
                ; syscall
                ; =>ok_label
            );
        }
        Expr::BinOp(Op2::Times, e1, e2) => {
            // Check if we know statically that either operand is not a number
            if let Some(true) = get_static_type(e1, env, define_env) {
                panic!("Invalid: invalid argument");
            }
            if let Some(true) = get_static_type(e2, env, define_env) {
                panic!("Invalid: invalid argument");
            }

            compile_ops(e1, ops, si, env, define_env, break_label);
            let stack_offset = si * 8;
            let error_label = ops.new_dynamic_label();

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset], rax
            );

            compile_ops(e2, ops, si+1, env, define_env, break_label);

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; sar rax, 1
                ; imul rax, [rsp - stack_offset]
            );

            let ok_label = ops.new_dynamic_label();
            dynasm!(ops ; .arch x64
                ; jmp =>ok_label
                ; =>error_label
                ; mov rdi, 1
                ; mov rax, 60
                ; syscall
                ; =>ok_label
            );
        }
        Expr::BinOp(Op2::Equal, e1, e2) => {
            // Check if we know statically that operands have different types
            let type1 = get_static_type(e1, env, define_env);
            let type2 = get_static_type(e2, env, define_env);

            if let (Some(t1), Some(t2)) = (type1, type2) {
                if t1 != t2 {
                    panic!("Invalid: invalid argument");
                }
            }

            compile_ops(e1, ops, si, env, define_env, break_label);
            let stack_offset = si * 8;
            dynasm!(ops ; .arch x64 ; mov [rsp - stack_offset], rax);
            compile_ops(e2, ops, si+1, env, define_env, break_label);

            let error_label = ops.new_dynamic_label();
            let ok_label = ops.new_dynamic_label();
            let stack_offset2 = (si + 1) * 8;
            dynasm!(ops ; .arch x64
                ; mov [rsp - stack_offset2], rax
                ; xor rax, [rsp - stack_offset]
                ; test rax, 1
                ; jnz =>error_label
                ; mov rax, [rsp - stack_offset2]
                ; cmp rax, [rsp - stack_offset]
                ; mov rax, 1
                ; mov QWORD [rsp - stack_offset], 3
                ; cmove rax, [rsp - stack_offset]
                ; jmp =>ok_label
                ; =>error_label
                ; mov rdi, 1
                ; mov rax, 60
                ; syscall
                ; =>ok_label
            );
        }
        Expr::BinOp(Op2::Greater, e1, e2) => {
            // Check if we know statically that either operand is not a number
            if let Some(true) = get_static_type(e1, env, define_env) {
                panic!("Invalid: invalid argument");
            }
            if let Some(true) = get_static_type(e2, env, define_env) {
                panic!("Invalid: invalid argument");
            }

            compile_ops(e1, ops, si, env, define_env, break_label);
            let stack_offset = si * 8;
            let stack_offset2 = (si + 1) * 8;
            let error_label = ops.new_dynamic_label();

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset], rax
            );

            compile_ops(e2, ops, si+1, env, define_env, break_label);

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset2], rax
                ; mov rax, [rsp - stack_offset]
                ; cmp rax, [rsp - stack_offset2]
                ; mov rax, 1
                ; mov QWORD [rsp - stack_offset], 3
                ; cmovg rax, [rsp - stack_offset]
            );

            let ok_label = ops.new_dynamic_label();
            dynasm!(ops ; .arch x64
                ; jmp =>ok_label
                ; =>error_label
                ; mov rdi, 1
                ; mov rax, 60
                ; syscall
                ; =>ok_label
            );
        }
        Expr::BinOp(Op2::GreaterEqual, e1, e2) => {
            // Check if we know statically that either operand is not a number
            if let Some(true) = get_static_type(e1, env, define_env) {
                panic!("Invalid: invalid argument");
            }
            if let Some(true) = get_static_type(e2, env, define_env) {
                panic!("Invalid: invalid argument");
            }

            compile_ops(e1, ops, si, env, define_env, break_label);
            let stack_offset = si * 8;
            let stack_offset2 = (si + 1) * 8;
            let error_label = ops.new_dynamic_label();

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset], rax
            );

            compile_ops(e2, ops, si+1, env, define_env, break_label);

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset2], rax
                ; mov rax, [rsp - stack_offset]
                ; cmp rax, [rsp - stack_offset2]
                ; mov rax, 1
                ; mov QWORD [rsp - stack_offset], 3
                ; cmovge rax, [rsp - stack_offset]
            );

            let ok_label = ops.new_dynamic_label();
            dynasm!(ops ; .arch x64
                ; jmp =>ok_label
                ; =>error_label
                ; mov rdi, 1
                ; mov rax, 60
                ; syscall
                ; =>ok_label
            );
        }
        Expr::BinOp(Op2::Less, e1, e2) => {
            // Check if we know statically that either operand is not a number
            if let Some(true) = get_static_type(e1, env, define_env) {
                panic!("Invalid: invalid argument");
            }
            if let Some(true) = get_static_type(e2, env, define_env) {
                panic!("Invalid: invalid argument");
            }

            compile_ops(e1, ops, si, env, define_env, break_label);
            let stack_offset = si * 8;
            let stack_offset2 = (si + 1) * 8;
            let error_label = ops.new_dynamic_label();

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset], rax
            );

            compile_ops(e2, ops, si+1, env, define_env, break_label);

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset2], rax
                ; mov rax, [rsp - stack_offset]
                ; cmp rax, [rsp - stack_offset2]
                ; mov rax, 1
                ; mov QWORD [rsp - stack_offset], 3
                ; cmovl rax, [rsp - stack_offset]
            );

            let ok_label = ops.new_dynamic_label();
            dynasm!(ops ; .arch x64
                ; jmp =>ok_label
                ; =>error_label
                ; mov rdi, 1
                ; mov rax, 60
                ; syscall
                ; =>ok_label
            );
        }
        Expr::BinOp(Op2::LessEqual, e1, e2) => {
            // Check if we know statically that either operand is not a number
            if let Some(true) = get_static_type(e1, env, define_env) {
                panic!("Invalid: invalid argument");
            }
            if let Some(true) = get_static_type(e2, env, define_env) {
                panic!("Invalid: invalid argument");
            }

            compile_ops(e1, ops, si, env, define_env, break_label);
            let stack_offset = si * 8;
            let stack_offset2 = (si + 1) * 8;
            let error_label = ops.new_dynamic_label();

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset], rax
            );

            compile_ops(e2, ops, si+1, env, define_env, break_label);

            dynasm!(ops ; .arch x64
                ; test rax, 1
                ; jnz =>error_label
                ; mov [rsp - stack_offset2], rax
                ; mov rax, [rsp - stack_offset]
                ; cmp rax, [rsp - stack_offset2]
                ; mov rax, 1
                ; mov QWORD [rsp - stack_offset], 3
                ; cmovle rax, [rsp - stack_offset]
            );

            let ok_label = ops.new_dynamic_label();
            dynasm!(ops ; .arch x64
                ; jmp =>ok_label
                ; =>error_label
                ; mov rdi, 1
                ; mov rax, 60
                ; syscall
                ; =>ok_label
            );
        }
        // ... rest of the match arms remain the same ...
        Expr::Let(bindings_vec, body) => {
            let mut new_env = env.clone();
            let mut current_si = si;

            for (var_name, binding_expr) in bindings_vec {
                compile_ops(binding_expr, ops, current_si, &new_env, define_env, break_label);
                let stack_offset = current_si * 8;
                dynasm!(ops ; .arch x64 ; mov [rsp - stack_offset], rax);
                new_env.insert(var_name.clone(), stack_offset);
                current_si += 1;
            }

            compile_ops(body, ops, current_si, &new_env, define_env, break_label);
        }
        Expr::Define(var, expr) => {
            compile_ops(expr, ops, si, env, define_env, break_label);
        }
        Expr::Loop(expr) => {
            let start_label = ops.new_dynamic_label();
            let end_label = ops.new_dynamic_label();
            dynasm!(ops ; .arch x64 ; =>start_label);
            compile_ops(expr, ops, si, env, define_env, Some(end_label));
            dynasm!(ops ; .arch x64
                ; jmp =>start_label
                ; =>end_label
            );
        }
        Expr::If(cond, true_code, false_code) => {
            compile_ops(cond, ops, si, env, define_env, break_label);
            let false_label = ops.new_dynamic_label();
            let done_label = ops.new_dynamic_label();
            dynasm!(ops ; .arch x64
                ; cmp rax, 1
                ; je =>false_label
            );
            compile_ops(true_code, ops, si, env, define_env, break_label);
            dynasm!(ops ; .arch x64 ; jmp =>done_label);
            dynasm!(ops ; .arch x64 ; =>false_label);
            compile_ops(false_code, ops, si, env, define_env, break_label);
            dynasm!(ops ; .arch x64 ; =>done_label);
        }
        Expr::Break(expr) => {
            compile_ops(expr, ops, si, env, define_env, break_label);
            if let Some(label) = break_label {
                dynasm!(ops ; .arch x64 ; jmp =>label);
            } else {
                panic!("Invalid: break outside of loop");
            }
        }
        Expr::Block(vec) => {
            for item in vec.iter() {
                compile_ops(item, ops, si, env, define_env, break_label);
            }
        }
        Expr::Set(name, e) => {
            compile_ops(e, ops, si, env, define_env, break_label);

            if let Some(stack_offset) = env.get(name) {
                dynasm!(ops ; .arch x64 ; mov [rsp - *stack_offset], rax);
            } else if let Some(def_val) = define_env.get(name) {
                match def_val {
                    DefineValue::Unknown(ptr) => {
                        let ptr_val = *ptr as i64;
                        dynasm!(ops ; .arch x64
                            ; mov rcx, QWORD ptr_val
                            ; mov [rcx], rax
                        );
                    }
                    DefineValue::Known(_) => {
                        panic!("Cannot set! a define'd variable that wasn't detected as mutable");
                    }
                }
            } else {
                panic!("Unbound variable identifier {}", name);
            }
        }
    }
}

// USED CLAUDE: Used claude to rewrite/format the existing helper function
// Helper function to check for keywords
// Prompt: Given this existing keyword function, could you reformat the function to look cleaner
fn is_keyword(name: &str) -> bool {
    matches!(name, 
        "true" | "false" | "input" | "let" | "add1" | "sub1" | "isnum" | "isbool" |
        "if" | "block" | "loop" | "break" | "set!" | "+" | "-" | "*" | "<" | ">" |
        ">=" | "<=" | "="
    )
} 
