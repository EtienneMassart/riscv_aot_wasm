(module
  (type (;0;) (func))
  (type (;1;) (func (result i64)))
  (func (;0;) (type 0)
    (local i32 i32 i32 i32 i32 i32 i32)
    i32.const 1024
    local.set 4
    loop  ;; label = @1
      i32.const 0
      local.set 5
      i32.const 1
      local.set 6
      local.get 2
      local.set 3
      i32.const 0
      local.set 0
      loop  ;; label = @2
        local.get 0
        local.get 4
        i32.add
        i32.const 999
        i32.const 999
        i32.const 999
        local.get 6
        local.get 5
        i32.const 7
        i32.div_u
        i32.const 7
        i32.mul
        i32.sub
        local.get 1
        local.get 3
        i32.const 7
        i32.div_u
        i32.const 28
        i32.mul
        i32.add
        local.get 0
        i32.eq
        select
        local.get 1
        local.get 3
        i32.const 13
        i32.div_u
        i32.const 52
        i32.mul
        i32.add
        local.get 0
        i32.eq
        select
        local.get 1
        local.get 3
        i32.const 11
        i32.div_u
        i32.const 44
        i32.mul
        i32.add
        local.get 0
        i32.eq
        select
        i32.store
        local.get 2
        local.get 5
        i32.add
        local.set 5
        local.get 2
        local.get 6
        i32.add
        local.set 6
        local.get 3
        i32.const 1
        i32.add
        local.set 3
        local.get 0
        i32.const 4
        i32.add
        local.tee 0
        i32.const 240
        i32.ne
        br_if 0 (;@2;)
      end
      local.get 4
      i32.const 240
      i32.add
      local.set 4
      local.get 1
      i32.const 4
      i32.sub
      local.set 1
      local.get 2
      i32.const 1
      i32.add
      local.tee 2
      i32.const 60
      i32.ne
      br_if 0 (;@1;)
    end)
  (func (;1;) (type 1) (result i64)
    (local i32 i32 i32 i32 i32)
    i32.const 31792
    local.tee 0
    i64.const 0
    i64.store offset=8
    i32.const 1024
    local.set 1
    loop  ;; label = @1
      i32.const 0
      local.set 2
      loop  ;; label = @2
        local.get 0
        local.get 1
        local.get 2
        i32.add
        local.tee 3
        i64.load32_s
        local.get 0
        i64.load offset=8
        i64.add
        i64.store offset=8
        local.get 0
        local.get 3
        i32.const 4
        i32.add
        i64.load32_s
        local.get 0
        i64.load offset=8
        i64.add
        i64.store offset=8
        local.get 0
        local.get 3
        i32.const 8
        i32.add
        i64.load32_s
        local.get 0
        i64.load offset=8
        i64.add
        i64.store offset=8
        local.get 0
        local.get 3
        i32.const 12
        i32.add
        i64.load32_s
        local.get 0
        i64.load offset=8
        i64.add
        i64.store offset=8
        local.get 2
        i32.const 16
        i32.add
        local.tee 2
        i32.const 240
        i32.ne
        br_if 0 (;@2;)
      end
      local.get 1
      i32.const 240
      i32.add
      local.set 1
      local.get 4
      i32.const 1
      i32.add
      local.tee 4
      i32.const 60
      i32.ne
      br_if 0 (;@1;)
    end
    local.get 0
    i64.load offset=8)
  (func (;2;) (type 0)
    (local i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32)
    i32.const 1024
    local.set 4
    loop  ;; label = @1
      local.get 3
      i32.const 2
      i32.shl
      local.set 8
      i32.const 1024
      local.set 5
      i32.const 0
      local.set 6
      loop  ;; label = @2
        local.get 6
        i32.const 240
        i32.mul
        local.get 8
        i32.add
        i32.const 1024
        i32.add
        local.set 7
        i32.const 0
        local.set 2
        loop  ;; label = @3
          local.get 2
          local.get 5
          i32.add
          local.tee 1
          i32.load
          local.set 0
          local.get 1
          local.get 0
          local.get 2
          local.get 4
          i32.add
          local.tee 9
          i32.load
          local.get 7
          i32.load
          i32.add
          local.tee 10
          local.get 0
          local.get 10
          i32.lt_s
          select
          i32.store
          local.get 1
          i32.const 4
          i32.add
          local.tee 0
          i32.load
          local.set 1
          local.get 0
          local.get 1
          local.get 9
          i32.const 4
          i32.add
          i32.load
          local.get 7
          i32.load
          i32.add
          local.tee 0
          local.get 0
          local.get 1
          i32.gt_s
          select
          i32.store
          local.get 2
          i32.const 8
          i32.add
          local.tee 2
          i32.const 240
          i32.ne
          br_if 0 (;@3;)
        end
        local.get 5
        i32.const 240
        i32.add
        local.set 5
        local.get 6
        i32.const 1
        i32.add
        local.tee 6
        i32.const 60
        i32.ne
        br_if 0 (;@2;)
      end
      local.get 4
      i32.const 240
      i32.add
      local.set 4
      local.get 3
      i32.const 1
      i32.add
      local.tee 3
      i32.const 60
      i32.ne
      br_if 0 (;@1;)
    end)
  (memory (;0;) 31808 32768 (pagesize 1))
  (export "memory" (memory 0))
  (export "init" (func 0))
  (export "result" (func 1))
  (export "kernel" (func 2)))
