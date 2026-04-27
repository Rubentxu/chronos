(module
  (func $add (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add)
  (func $multiply (export "multiply") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.mul)
  (func $fibonacci (export "fibonacci") (param i32) (result i32)
    (if (i32.le_s (local.get 0) (i32.const 1))
      (then (return (local.get 0))))
    (i32.add
      (call $fibonacci (i32.sub (local.get 0) (i32.const 1)))
      (call $fibonacci (i32.sub (local.get 0) (i32.const 2)))))
)