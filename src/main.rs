use std::rc::Rc;

pub mod vm;

use vm::*;

fn main() {
    use Op::*;
    let ops = vec![
        Op::Const(Data::Float(3.0)),
        Op::Const(Data::Float(4.0)),
        Op::Const(Data::Float(5.0)),
        Op::Add,
        Op::Div,
        Op::Const(Data::Fun(Fun {
            ops: Rc::new(vec![
                Op::Call,
            ]),
            captures: Rc::new(vec![]),
        })),
        Op::Handler(Name(0)),
        Op::Raise(Name(0)),
    ];

    let fun = Fun {
        ops: Rc::new(ops),
        captures: Rc::new(vec![]),
    };

    let mut fiber = Fiber::new(fun);
    println!("Result: {:#?}", fiber.run());

    println!("Fiber: {:#?}", fiber);
}
