extern crate cretonne;
extern crate cretonne_module;
extern crate cretonne_faerie;

use std::process;

mod jit;

fn main() {
    // Create the JIT instance, which manages all generated functions and data.
    let mut jit = jit::JIT::new("test.o");

    // A small test function.
    //
    // The `(c)` declares a return variable; the function returns whatever value
    // it was assigned when the function exits. Note that there are multiple
    // assignments, so the input is not in SSA form, but that's ok because
    // Cretonne handles all the details of translating into SSA form itself.
    let foo_code = "\
        fn foo(a, b) -> (c) {      \n\
            c = if a {             \n\
                if b {             \n\
                    30             \n\
                } else {           \n\
                    40             \n\
                }                  \n\
            } else {               \n\
                50                 \n\
            }                      \n\
            c = c + 2              \n\
        }                          \n\
    ";

    // Pass the string to the JIT, and it returns a raw pointer to machine code.
    jit.compile(&foo_code).unwrap_or_else(|msg| {
        eprintln!("error: {}", msg);
        process::exit(1);
    });

    // -------------------------------------------------------------------------//

    // Another example: Recursive fibonacci.
    let recursive_fib_code = "\
        fn recursive_fib(n) -> (r) {                                      \n\
            r = if n == 0 {                                               \n\
                     0                                                    \n\
                } else {                                                  \n\
                    if n == 1 {                                           \n\
                        1                                                 \n\
                    } else {                                              \n\
                        recursive_fib(n - 1) + recursive_fib(n - 2)       \n\
                    }                                                     \n\
                }                                                         \n\
        }                                                                 \n\
    ";

    // Same as above.
    jit.compile(&recursive_fib_code).unwrap_or_else(|msg| {
        eprintln!("error: {}", msg);
        process::exit(1);
    });

    // -------------------------------------------------------------------------//

    // Another example: Iterative fibonacci.
    let iterative_fib_code = "\
        fn iterative_fib(n) -> (r) {                                      \n\
            if n == 0 {                                                   \n\
                r = 0                                                     \n\
            } else {                                                      \n\
                n = n - 1                                                 \n\
                a = 0                                                     \n\
                r = 1                                                     \n\
                while n != 0 {                                            \n\
                    t = r                                                 \n\
                    r = r + a                                             \n\
                    a = t                                                 \n\
                    n = n - 1                                             \n\
                }                                                         \n\
            }                                                             \n\
        }                                                                 \n\
    ";

    // Same as above.
    jit.compile(&iterative_fib_code).unwrap_or_else(|msg| {
        eprintln!("error: {}", msg);
        process::exit(1);
    });

    // -------------------------------------------------------------------------//

    // Let's say hello, by calling into libc. The puts function is resolved by
    // dlsym to the libc function, and the string &hello_string is defined below.
    let hello_code = "\
        fn hello() -> (r) {                                               \n\
            puts(&hello_string)                                           \n\
        }                                                                 \n\
    ";

    jit.create_data("hello_string", "hello world\0".as_bytes().to_vec())
        .unwrap_or_else(|msg| {
            eprintln!("error: {}", msg);
            process::exit(1);
        });

    // Same as above.
    jit.compile(&hello_code).unwrap_or_else(|msg| {
        eprintln!("error: {}", msg);
        process::exit(1);
    });

    // -------------------------------------------------------------------------//

    // Now let's write a main function, and call all the functions we just
    // compiled. For now, this doesn't print the computed values; only the
    // hello function prints something.
    let main_code = "\
        fn main(argc, argv) -> (r) {                                      \n\
            foo(1, 0)                                                     \n\
            recursive_fib(10)                                             \n\
            iterative_fib(10)                                             \n\
            hello()                                                       \n\
        }                                                                 \n\
    ";

    // Same as above.
    jit.compile(&main_code).unwrap_or_else(|msg| {
        eprintln!("error: {}", msg);
        process::exit(1);
    });

    // Now write out a .o file!
    jit.finish();
}
