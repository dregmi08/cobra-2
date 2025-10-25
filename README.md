[![Review Assignment Due Date](https://classroom.github.com/assets/deadline-readme-button-22041afd0340ce965d47ae6ef1cefeee28c7c493a6346c4f15d667ab976d596c.svg)](https://classroom.github.com/a/hQU87gZn)
[![Open in Codespaces](https://classroom.github.com/assets/launch-codespace-2972f46106e565e64193e422d61a12cf1da4916b45550586e14ef0a7c637dd04.svg)](https://classroom.github.com/open-in-codespaces?assignment_repo_id=21243540)
Im not sure why the autograder test fails in github actions but everything works fine when I run `cargo build` and `cargo test` locally, here is the example output with two tests:

```
   = note: `#[warn(dead_code)]` on by default

warning: `cobra` (test "all_tests") generated 6 warnings (run `cargo fix --test "all_tests"` to apply 4 suggestions)
warning: `cobra` (bin "cobra" test) generated 6 warnings (6 duplicates)
    Finished test [unoptimized + debuginfo] target(s) in 0.29s
     Running unittests src/main.rs (target/debug/deps/cobra-b842a7e7f5e97b2e)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/all_tests.rs (target/debug/deps/all_tests-58ce49e28d7adda3)

running 2 tests
JIT result: 101

test add ... ok
[repl_test] Success!
Expected vector: ["true", "false"]
Actual vector:   ["true", "false"]

test test_simple_bools ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.30s

[dregmi@ieng6-203]:cobra-dregmi08:506$ 

```

