Hello!

This is a simple demo that JIT-compiles a toy language, using Cretonne.

It uses the new SimpleJIT interface in development
[here](https://github.com/sunfishcode/cretonne/tree/module). SimpleJIT takes care
of managing a symbol table, allocating memory, and performing relocations, offering
a relatively simple API.

This is inspired in part by Ulysse Carion's
[llvm-rust-getting-started](https://github.com/ucarion/llvm-rust-getting-started)
and Jonathan Turner's [rustyjit](https://github.com/jonathandturner/rustyjit).

A quick introduction to Cretonne: Cretonne is a compiler backend. It's
light-weight, supports `no_std` mode, doesn't use of floating-point itself,
and it makes efficient use of memory.

And Cretonne is being architected to allow flexibility in how one uses it.
Sometimes that flexibility can be a burden, which we've recently started to
address in a new set of crates, `cretonne-module`, `cretonne-simplejit`, and
`cretonne-faerie`, which put the pieces together in some easy-to-use
configurations for working with multiple functions at once. `cretonne-module`
is a common interface for working with multiple functions and data interfaces
at once. This interface can sit on top of `cretonne-simplejit`, which writes
code and data to memory where they can be executed and accessed. And, it can
sit on top of `cretonne-faerie`, which writes code and data to native .o files
which can be linked into native executables.

This post introduces Cretonne by walking through a simple JIT demo, using
the [`cretonne-simplejit`](https://crates.io/crates/cretonne-simplejit) crate.
Currently this demo works on Linux x86-64 platforms. It may also work on Mac
x86-64 platforms, though I haven't specifically tested that yet. And Cretonne
is being designed to support many other kinds of platforms in the future.

### A walkthrough

First, let's take a quick look at the toy language in use. It's a very
simple language, in which all variables have type `isize`. (Cretonne does have
full support for other integer and floating-point types, so this is just to
keep the toy language simple).

For a quick flavor, here's our
[first example](https://github.com/sunfishcode/simplejit-demo/blob/master/src/toy.rs#L21)
in the toy language:

```
        fn foo(a, b) -> (c) {
            c = if a {
                if b {
                    30
                } else {
                    40
                }
            } else {
                50
            }
            c = c + 2
        }
```

The grammar for this toy language is defined in a grammar file
[here](https://github.com/sunfishcode/simplejit-demo/blob/master/src/grammar.rustpeg),
and this demo uses the [peg](https://crates.io/crates/peg) parser generator library
to generate actual parser code for it.

The output of parsing is a [custom AST type](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L13):
```rust
pub enum Expr {
    Literal(String),
    Identifier(String),
    Assign(String, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    IfElse(Box<Expr>, Vec<Expr>, Vec<Expr>),
    WhileLoop(Box<Expr>, Vec<Expr>),
    Call(String, Vec<Expr>),
    GlobalDataAddr(String),
}
```

It's pretty minimal and straightforward. The `IfElse` can return a value, to show
how that's done in Cretonne (see below).

The
[first thing we do](https://github.com/sunfishcode/simplejit-demo/blob/master/src/toy.rs#L12)
is create an instance of our `JIT`:
```rust
let mut jit = jit::JIT::new();
```

The `JIT` class is defined
[here](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L40)
and contains several fields:
 - `builder_context` - Cretonne uses this to reuse dynamic allocations between
   compiling multiple functions.
 - `ctx` - This is the main `Context` object for compiling functions.
 - `data_ctx` - Similar to `ctx`, but for "compiling" data sections.
 - `module` - The `Module` which holds information about all functions and data
   objects defined in the current `JIT`.

Before we go any further, let's talk about the underlying model here. The
`Module` class divides the world into two kinds of things: functions, and data
objects. Both functions and data objects have *names*, and can be imported into
a module, defined and only referenced locally, or defined and exported for use
in outside code. Functions are immutable, while data objects can be declared
either readonly or writable.

Both functions and data objects can contain references to other functions and
data objects. Cretonne is designed to allow the low-level parts operate on each
function and data object independently, so each function and data object maintains
its own individual namespace of imported names. The
[`Module`](https://docs.rs/cretonne-module/0.6.0/cretonne_module/struct.Module.html)
struct takes care of maintaining a set of declarations for use across multiple
functions and data objects.

These concepts are sufficiently general that they're applicable to JITing as
well as native object files (more discussion below!), and `Module` provides an
interface which abstracts over both. It is parameterized with a
[`Backend`](https://docs.rs/cretonne-module/0.6.0/cretonne_module/trait.Backend.html)
trait, which allows users to specify what underlying implementation they want to use.

Once we've
[initialized the JIT data structures](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L64),
we then use our `JIT` to
[compile](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L73)
some functions.

The `JIT`'s `compile` function takes a string containing a function in the
toy language. It
[parses](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L75)
the string into an AST, and then
[translates](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L79)
the AST into Cretonne IR.

Our toy language only supports one type, so we start by
[declaring that type](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L143)
for convenience.

We then we start translating the function by adding
[the function parameters](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L147)
and
[return types](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L151)
to the Cretonne function signature.

Then we
[create](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L155)
a
[FunctionBuilder](https://docs.rs/cretonne-frontend/0.6.0/cretonne_frontend/struct.FunctionBuilder.html)
which is a utility for building up the contents of a Cretonne IR function. As we'll
see below, `FunctionBuilder` includes functionality for constructing SSA form
automatically so that users don't have to worry about it.

Next, we
[start](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L159)
an initial extended basic block (EBB), which is the entry block of the function, and
the place where we'll insert some code.

 - A basic block is a sequence of IR instructions which have a single entry
   point, and no branches until the very end, so execution always starts at the
   top and proceeds straight through to the end.

 - Cretonne IR uses *extended* basic blocks, which are similar to basic blocks,
   except they may branch out from the middle.

   TODO: add an illustration here

Cretonne's extended basic blocks can have parameters. These take the place of
PHI functions in other IRs. The `FunctionBuilder` library will take care of
inserting EBB parameters automatically, so frontends that don't need to use
them directly generally don't need to worry about them, though one place they
do come up is that incoming arguments to a function are represented as
EBB parameters to the entry block. We must tell Cretonne to add the parameters,
using
[`append_ebb_params_for_function_params`](https://docs.rs/cretonne-frontend/0.6.0/cretonne_frontend/struct.FunctionBuilder.html#method.append_ebb_params_for_function_params)
like
[so](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L162).

The `FunctionBuilder` keeps track of a "current" EBB that new instructions are
to be inserted into; we next
[inform](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L168)
it of our new block, using
[`switch_to_block`](https://docs.rs/cretonne-frontend/0.6.0/cretonne_frontend/struct.FunctionBuilder.html#method.switch_to_block),
so that we can start
inserting instructions into it.

The one major concept about EBBs is that the `FunctionBuilder` wants to know when
all branches which could branch to an EBB have been seen, at which point it can
*seal* the EBB, which allows it to perform SSA construction. All EBBs must be
sealed by the end of the function. We
[seal](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L171)
an EBB with
[`seal_block`](https://docs.rs/cretonne-frontend/0.6.0/cretonne_frontend/struct.FunctionBuilder.html#method.seal_block).

Next, our toy language doesn't have explicit variable declarations, so we walk the
AST to discover all the variables, so that we can
[declare](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L176)
then to the `FunctionBuilder`. These variables need not be in SSA form; the
`FunctionBuilder` will take care of constructing SSA form internally.

For convenience when walking the function body, the demo here
[uses](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L181)
 a `FunctionTranslator` object, which holds the `FunctionBuilder`, the current
`Module`, as well as the symbol table for looking up variables. Now we can start
[walking the function body](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L188).

[AST translation](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L219)
utilizes the instruction-building features of `FunctionBuilder`. Let's start with
a simple example translating integer literals:

```rust
    Expr::Literal(literal) => {
        let imm: i32 = literal.parse().unwrap();
        self.builder.ins().iconst(self.int, i64::from(imm))
    }
```

The first part is just extracting the integer value from the AST. The next
line is the builder line:

 - The `.ins()` returns an "insertion object", which allows inserting an
   instruction at the end of the currently active EBB.
 - `iconst` is the name of the builder routine for creating
   [integer constants](https://cretonne.readthedocs.io/en/latest/langref.html#inst-iconst)
   in Cretonne. Every instruction in the IR can be created directly through such
   a function call.

Translation of
[Add nodes](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L226)
and other arithmetic operations is similarly straightforward.

Translation of
[variable references](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L304)
is mostly handled by `FunctionBuilder`'s `use_var` function:
```rust
    Expr::Identifier(name) => {
        // `use_var` is used to read the value of a variable.
        let variable = self.variables.get(&name).expect("variable not defined");
        self.builder.use_var(*variable)
    }
```
`use_var` is for reading the value of a (non-SSA) variable. (Internally,
`FunctionBuilder` constructs SSA form to satisfy all uses).

Its companion is `def_var`, which is used to write the value of a (non-SSA)
variable, which we use to implement assignment:
```
    Expr::Assign(name, expr) => {
        // `def_var` is used to write the value of a variable. Note that
        // variables can have multiple definitions. Cretonne will
        // convert them into SSA form for itself automatically.
        let new_value = self.translate_expr(*expr);
        let variable = self.variables.get(&name).unwrap();
        self.builder.def_var(*variable, new_value);
        new_value
    }
```

Next, let's dive into
[if-else](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L320)
expressions. In order to demonstrate explicit SSA construction, this demo gives
if-else expressions return values. The way this looks in Cretonne is that
The true and false arms of the if-else both have branches to a common merge point,
and they each pass their "return value" as an EBB parameter to the merge point.

Note that we seal the EBBs we create once we know we'll have no more predecessors,
which is something that a typical AST makes it easy to know.

TODO: Show the cton IR here.

The [while loop](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L367)
translation is also straightforward.

TODO: Show the cton IR here.

TODO:
[calls](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L394)

TODO:
[global data symbols](https://github.com/sunfishcode/simplejit-demo/blob/master/src/jit.rs#L422)

And with that, we can return to our main `toy.rs` file and run some more examples.

TODO: recursive fib, iterative fib

And to show off a handy feature of the simplejit backend, it can look up symbols
with `libc::dlsym`, so you can call libc functions such as `puts` (being careful
to NUL-terminate your strings!). Unfortunately, `printf` requires varargs, which
Cretonne does not yet support.

TODO: talk about defining the hello world string data object.

And so we can say, "hello world"!.


### Native object files

Because of the `Module` abstraction, this demo can be adapted to write out an ELF
.o file rather than JITing the code to memory with only minor changes, and I've done
so in a branch [here](https://github.com/sunfishcode/simplejit-demo/tree/faerie).
This writes a `test.o` file, which on an x86-64 ELF platform you can link with
`cc test.o` and it produces an executable that calls the generated functions,
including printing "hello world"!.

Object files are written using the
[faerie](https://github.com/m4b/faerie) library, which also has Mach-O support
at this time, though I haven't tried it yet.

### Have fun!

Cretonne is still evolving, so if there are things here which are confusing or
awkward, please let us know, via
[githib issues](https://github.com/cretonne/cretonne/issues) or
just stop by the [gitter chat](https://gitter.im/cretonne/Lobby/~chat).
Very few things in Cretonne's design are set in stone at this time, and we're
really interested to hear from people about what makes sense what doesn't.

TODO: update llvm2cretonne for api changes

TODO: update https://github.com/cretonne/cretonne-getting-started for api changes

TODO: update wasmstandalone for api changes
