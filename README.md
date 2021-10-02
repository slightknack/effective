# A Simple Virtual Machine with Effects

Each thread of execution in this VM is called a `Fiber`. A Fiber is unique, can be sent between threads, but can not be copied. It has:

- A local stack
- Some bytecode
- A program counter

Which is pretty neat.

An *effect* is basically a dynamically-scoped function. *System injection* is leveraging effects so that the host runtime can interact with the VM.

# A simple example
Here's the stack of an example fiber running a program—the `[X]` denote stack frames

```
[A] 1 2 [B] 3 4
```

Let's say we want to print `4` using system injection. First thing we do is execute the `print` effect. This checks the frames `[A]` and `[B]` for handlers. Not surprisingly, there are none.

For this reason, `4` is popped off the stack and wrapped up with the name of the effect; the `Fiber` itself then returns this wrapped `print|4` effect. This is then handled by the runtime, and the `Fiber` is then resumed by passing in a value. Let's say printing returns zero:

```
[A] 1 2 [B] 3 4 -- initial stack
[A] 1 2 [B] 3   -- system injection
[A] 1 2 [B] 3 0 -- new stack
```

# Introducing handlers
Beautiful. Now, let's say that we are back to the original stack, and want to print again. This time, however, `[A]` has registered an effect *handler*. We've denoted the fact that `[A]` now has a handler with a `*`:

```
[A*] 1 2 [B] 3 4
```

Wow! A handler is just a function that takes a value, and ultimately either returns another value or resumes with a value.

Let's start with the easier of the two cases, that the effect just resumes:

```
[A*]:
    print = v -> resume (v + 1)
```

Let's walk through the call to print again, with this new definition of `print` in place:

```
[A*] 1 2 [B] 3 4
```

Let's say we want to print `4` using effect handling. First thing we do is execute the `print` effect. This checks the frames `[A*]` and `[B]` for handlers. This time, we find a `print` handler on frame `[A*]`.

For this reason, `4` is popped off the stack. We then create a *new* fiber, and initialize it with the print handler function:

```
[A*] 1 2 [B] 3 4 -- initial stack

[A*] 1 2 [B] 3 -- not active
[C] 4          -- active handler stack
```

We then run this function:

```
[A*] 1 2 [B] 3 -- not active
[C] 5          -- active after `(v + 1)`
```

And then invoke `resume`. When `resume` is invoked, we pop `5` off the stack, destroy the handling stack, push `5` on to the original stack, and resume execution:

```
[A*] 1 2 [B] 3 -- not active
[C]            -- resume 5

[A*] 1 2 [B] 3 5 -- active
```

Resume mirrors the path of a system injection, only the system that is handling the code is not the parent system.

# Something a bit more complex
This is the trivial case; let us now discuss the more complex case, when `resume` is not invoked and a value is returned instead.

Here's the original stack, yet again:

```
[A*] 1 2 [B] 3 4
```

And the handler is now:

```
[A*]:
    print = v -> (v - 1)
```

Note that the handler does not resume and instead returns the value minus one. With this new handler in mind, let's simulate the system:

Let's say we want to print `4` using effect handling. First thing we do is execute the `print` effect. This checks the frames `[A*]` and `[B]` for handlers. This time, we find a `print` handler on frame `[A*]`.

For this reason, `4` is popped off the stack. We then create a *new* fiber, and, like before, initialize it with the print handler function:

```
[A*] 1 2 [B] 3 4 -- initial stack

[A*] 1 2 [B] 3 -- not active
[C] 4          -- active handler stack

[A*] 1 2 [B] 3 -- not active
[C] 3          -- active after (v - 1)
```

No surprises so far; we now execute the return instruction, like this were a normal function:

```
[A*] 1 2 [B] 3 -- not active
3              -- returned
```

Returning just replaces the topmost stack frame with the value to be returned. This is true in all cases.

So now, the question becomes, what happens next?

When an effect is called, we keep track of the stack frame that caused the effect to be raised. so, for instance:

```
[A*] 1 2 [B] 3
[C:A*] 3
```

The stack frame `[C]` was created because of an effect defined in `[A*]`, hence: `[C:A*]`.

When we return, we check whether the stack frame was created because of an effect. because `[C]` was obviously created because of an effect, instead of returning from `[C]`, we instead return from `[A*]`:

```
[A*] 1 2 [B] 3 -- first stack
[C:A*] 3       -- before return

3              -- first stack after return
```

Alright, so the clever among you may have noticed something: The second stack for handling the effect runs once, then is destroyed; the stack below it is never modified.

For this reason, the combination of effects and handlers, with the resume operation, create delimited continuations, which can be used to implement control flow.

# Fibers
