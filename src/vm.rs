use std::{
    rc::Rc,
    collections::BTreeMap,
};

#[derive(Debug, Clone, Copy, PartialOrd, Ord, Eq, PartialEq)]
pub struct Name(pub usize);

#[derive(Debug, Clone)]
pub enum Op {
    Return(usize),
    Call,
    Const(Data),
    Add,
    Div,
    Get(Name),
    Set(Name),
    Handler(Name),
    Raise(Name),
    Pop(usize),
    // Resume,
}

#[derive(Debug, Clone)]
pub struct RawFun {
    ops: Rc<Vec<Op>>,
    num_captures: usize,
}

/// Represents a function before execution
#[derive(Debug, Clone)]
pub struct Fun {
    pub ops:      Rc<Vec<Op>>,
    pub captures: Rc<Vec<Data>>,
}

#[derive(Debug)]
pub struct Suspend {
    ops: Rc<Vec<Op>>,
    pc:  usize,
}

impl Suspend {
    pub fn new(ops: Rc<Vec<Op>>, pc: usize) -> Suspend {
        Suspend { ops, pc }
    }
}

/// Represents a single function in the process of execution
#[derive(Debug)]
struct Frame {
    suspend:  Option<Suspend>,
    index:    usize, // index of data on stack, i.e. where this frame is.
    captures: Rc<Vec<Data>>,
    handlers: BTreeMap<Name, Fun>,
}

impl Frame {
    pub fn new(
        suspend: Option<Suspend>,
        index: usize,
        captures: Rc<Vec<Data>>,
    ) -> Frame {
        Frame {
            suspend,
            index,
            captures,
            handlers: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Data {
    Float(f64),
    RawFun(RawFun),
    Fun(Fun),
    Cont(Rc<Fiber>),
}

impl Data {
    fn try_math(
        self,
        other: Self,
        binop: fn(f64, f64) -> Result<f64, Effect>
    ) -> Result<Data, Effect> {
        match (self, other) {
            (Data::Float(a), Data::Float(b)) => {
                Ok(Data::Float(binop(a, b)?))
            },
            _ => Err(Effect::TypeMismatch),
        }
    }
}

#[derive(Debug)]
struct Stack {
    datum: Vec<Data>,
    frames: Vec<Frame>,
}

impl Stack {
    pub fn new(captures: Rc<Vec<Data>>) -> Stack {
        Stack {
            datum: vec![],
            frames: vec![Frame::new(None, 0, captures)],
        }
    }
}

#[derive(Debug)]
pub enum Effect {
    /// Errors that indicate invalid opcode
    Fatal,
    TypeMismatch,
    ZeroDivision,
    Virtual(Name, Data),
}

/// Represents a stack of functions in the process of being executed
#[derive(Debug)]
pub struct Fiber {
    parent: Option<Rc<Fiber>>,
    stack: Stack,
    ops:   Rc<Vec<Op>>,
    pc:    usize,
}

impl Fiber {
    pub fn new(fun: Fun) -> Fiber {
        Fiber {
            parent: None,
            stack:  Stack::new(fun.captures),
            ops:    fun.ops,
            pc:     0,
        }
    }

    fn push(&mut self, data: Data) {
        self.stack.datum.push(data)
    }

    fn kill(&mut self) {
        self.pc = self.ops.len();
    }

    fn is_done(&self) -> bool {
        self.pc >= self.ops.len()
    }

    fn next_op(&self) -> Op {
        self.ops[self.pc].clone()
    }

    /// unwraps an item or returns the Fatal effect and kills the fiber
    fn unwrap_or_fatal<T>(&mut self, item: Option<T>) -> Result<T, Effect> {
        match item {
            Some(valid) => Ok(valid),
            None => {
                self.kill();
                Err(Effect::Fatal)
            },
        }
    }

    fn pop(&mut self) -> Result<Data, Effect> {
        let top = self.stack.datum.pop();
        self.unwrap_or_fatal(top)
    }

    fn resolve_handler<T>(
        &self,
        name: Name,
        extract: impl Fn(&Frame) -> T,
    ) -> Option<T> {
        for frame in self.stack.frames.iter().rev() {
            if let Some(_) = frame.handlers.get(&name) {
                return Some(extract(frame));
            }
        }

        if let Some(parent) = &self.parent {
            parent.resolve_handler(name, extract)
        } else {
            None
        }
    }

    pub fn run(&mut self) -> Result<(), Effect> {
        use Op::*;

        while !self.is_done() {
            println!("Before: {:#?}", self);

            match self.next_op() {
                Const(data) => {
                    self.push(data.clone());
                },

                Add => {
                    let a = self.pop()?;
                    let b = self.pop()?;
                    self.push(Data::try_math(
                        a, b,
                        |a, b| Ok(a + b),
                    )?)
                },

                Div => {
                    let a = self.pop()?;
                    let b = self.pop()?;
                    self.push(Data::try_math(
                        a, b,
                        |a, b| if a == 0.0 {
                            Err(Effect::ZeroDivision)
                        } else {
                            Ok(a / b)
                        },
                    )?)
                },

                Handler(name) => {
                    let fp = match self.pop()? {
                        Data::Fun(f) => f,
                        _ => Err(Effect::TypeMismatch)?,
                    };

                    let mut frames = std::mem::take(&mut self.stack.frames);
                    self.unwrap_or_fatal(frames.last_mut())?
                        .handlers.insert(name, fp);
                    std::mem::swap(&mut self.stack.frames, &mut frames);
                },

                Raise(name) => {
                    let fun = self.resolve_handler(
                        name,
                        |frame| frame.handlers.get(&name).unwrap().clone(),
                    );

                    let data = self.pop()?;
                    let fun = match fun {
                        Some(f) => f,
                        None => Err(Effect::Virtual(name, data.clone()))?,
                    };

                    let new_fiber = Fiber::new(fun);
                    self.switch(new_fiber, data);
                    continue;
                }

                Call => {
                    let arg = self.pop()?;
                    let fun = self.pop()?;

                    match fun {
                        Data::Fun(fun) => {
                            self.call(fun);
                            self.push(arg);
                        }
                        Data::Cont(fiber) => {
                            let fiber = self.unwrap_or_fatal(Rc::<Fiber>::try_unwrap(fiber).ok())?;
                            self.switch(fiber, arg);
                        }
                        _ => Err(Effect::TypeMismatch)?,
                    }
                }

                Pop(times) => {
                    for _ in 0..times {
                        self.pop()?;
                    }
                }

                Capture => {
                    let raw_fun = match self.pop()? {
                        Data::RawFun(r) => r,
                        _ => Err(Effect::TypeMismatch)?,
                    };

                    self.stack.datum.split_off(
                        self.stack.datum.len().try_sub(raw_fun.num_captures)
                    )

                    todo!()
                }

                _ => todo!(),
            }

            println!("After: {:#?}", self);
            self.pc += 1;
        }

        Ok(())
    }

    pub fn switch(&mut self, other_fiber: Fiber, data: Data) {
        let old_fiber = std::mem::replace(self, other_fiber);
        let cont = Data::Cont(Rc::new(old_fiber));
        self.push(cont);
        self.push(data);
    }

    pub fn call(&mut self, fun: Fun) {
        let old_ops = std::mem::replace(&mut self.ops, fun.ops);
        let old_pc  = std::mem::replace(&mut self.pc,  0);
        let suspend = Suspend::new(old_ops, old_pc);

        let frame = Frame::new(
            Some(suspend),
            self.stack.datum.len(),
            fun.captures,
        );
        self.stack.frames.push(frame);
    }
}
