(module
  (type (;0;) (func))
  (type (;1;) (func (result i64)))
  (func (;0;) (type 0)
    (local i32)
    i32.const 1080
    i32.const 197121
    i32.store
    i32.const 1072
    i64.const 846628248551937
    i64.store
    i32.const 1064
    i64.const 846628248551937
    i64.store
    i32.const 1056
    i64.const 846628248551937
    i64.store
    i32.const 1048
    i64.const 846628248551937
    i64.store
    i32.const 1040
    i64.const 846628248551937
    i64.store
    i32.const 1032
    i64.const 846628248551937
    i64.store
    i32.const 1024
    i64.const 846628248551937
    i64.store
    loop  ;; label = @1
      local.get 0
      i32.const 1320
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1312
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1304
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1296
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1288
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1280
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1272
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1264
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1256
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1248
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1240
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1232
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1224
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1216
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1208
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1200
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1192
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1184
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1176
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1168
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1160
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1152
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1144
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1136
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1128
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1120
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1112
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1104
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1096
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 1088
      i32.add
      i64.const 0
      i64.store
      local.get 0
      i32.const 240
      i32.add
      local.tee 0
      i32.const 14400
      i32.ne
      br_if 0 (;@1;)
    end)
  (func (;1;) (type 1) (result i64)
    (local i32 i32 i32 i32 i32 i32 i32 i32)
    i32.const 31856
    local.tee 0
    i64.const 0
    i64.store offset=8
    i32.const 1088
    local.set 4
    i32.const 1088
    local.set 5
    loop  ;; label = @1
      i32.const 0
      local.get 3
      local.tee 1
      i32.sub
      i32.const 3
      i32.and
      if  ;; label = @2
        i32.const 0
        local.get 6
        i32.const 3
        i32.and
        i32.sub
        local.set 7
        i32.const 0
        local.set 2
        local.get 5
        local.set 1
        loop  ;; label = @3
          local.get 0
          local.get 1
          i64.load32_s
          local.get 0
          i64.load offset=8
          i64.add
          i64.store offset=8
          local.get 1
          i32.const 4
          i32.add
          local.set 1
          local.get 2
          i32.const 1
          i32.sub
          local.tee 2
          local.get 7
          i32.ne
          br_if 0 (;@3;)
        end
        local.get 3
        local.get 2
        i32.sub
        local.set 1
      end
      local.get 3
      i32.const 57
      i32.sub
      i32.const 3
      i32.ge_u
      if  ;; label = @2
        local.get 1
        i32.const 2
        i32.shl
        local.set 2
        loop  ;; label = @3
          local.get 0
          local.get 2
          local.get 4
          i32.add
          local.tee 1
          i64.load32_s
          local.get 0
          i64.load offset=8
          i64.add
          i64.store offset=8
          local.get 0
          local.get 1
          i32.const 4
          i32.add
          i64.load32_s
          local.get 0
          i64.load offset=8
          i64.add
          i64.store offset=8
          local.get 0
          local.get 1
          i32.const 8
          i32.add
          i64.load32_s
          local.get 0
          i64.load offset=8
          i64.add
          i64.store offset=8
          local.get 0
          local.get 1
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
          br_if 0 (;@3;)
        end
      end
      local.get 4
      i32.const 240
      i32.add
      local.set 4
      local.get 6
      i32.const 3
      i32.add
      local.set 6
      local.get 5
      i32.const 244
      i32.add
      local.set 5
      local.get 3
      i32.const 1
      i32.add
      local.tee 3
      i32.const 60
      i32.ne
      br_if 0 (;@1;)
    end
    local.get 0
    i64.load offset=8)
  (func (;2;) (type 0)
    (local i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32)
    i32.const 59
    local.set 0
    i32.const 1568
    local.set 9
    i32.const 1088
    local.set 10
    loop  ;; label = @1
      local.get 0
      local.tee 4
      i32.const 58
      i32.le_u
      if  ;; label = @2
        local.get 0
        i32.const 1
        i32.add
        local.tee 6
        i32.const 2
        i32.shl
        local.get 0
        i32.const 240
        i32.mul
        local.tee 0
        i32.const 1088
        i32.add
        local.tee 14
        i32.add
        local.set 15
        local.get 6
        i32.const 240
        i32.mul
        i32.const 1088
        i32.add
        local.tee 16
        local.get 4
        i32.const 2
        i32.shl
        local.tee 1
        i32.add
        i32.load
        local.set 11
        local.get 1
        local.get 14
        i32.add
        i32.load
        local.set 2
        local.get 4
        i32.const 2
        i32.add
        local.set 17
        local.get 0
        i32.const 480
        i32.add
        local.set 18
        i32.const 0
        local.set 7
        local.get 9
        local.set 12
        local.get 6
        local.set 3
        loop  ;; label = @3
          local.get 11
          local.set 0
          local.get 3
          i32.const 2
          i32.shl
          local.tee 5
          local.get 14
          i32.add
          local.tee 8
          i32.load
          local.tee 1
          local.get 2
          local.get 1
          local.get 2
          i32.gt_s
          select
          local.tee 1
          local.get 5
          local.get 16
          i32.add
          i32.load
          local.tee 11
          local.get 1
          local.get 11
          i32.gt_s
          select
          local.set 1
          local.get 3
          i32.const 1
          i32.sub
          local.get 4
          i32.gt_u
          if  ;; label = @4
            local.get 0
            local.get 3
            i32.const 1024
            i32.add
            i32.load8_s
            local.get 4
            i32.const 1024
            i32.add
            i32.load8_s
            i32.add
            i32.const 3
            i32.eq
            i32.add
            local.set 0
          end
          local.get 8
          local.get 1
          local.get 0
          local.get 0
          local.get 1
          i32.lt_s
          select
          local.tee 2
          i32.store
          block  ;; label = @4
            local.get 3
            local.get 6
            i32.le_u
            br_if 0 (;@4;)
            local.get 7
            i32.const 1
            i32.and
            if (result i32)  ;; label = @5
              local.get 8
              local.get 2
              local.get 5
              i32.const 1088
              i32.add
              local.get 18
              i32.add
              i32.load
              local.get 15
              i32.load
              i32.add
              local.tee 1
              local.get 1
              local.get 2
              i32.lt_s
              select
              local.tee 2
              i32.store
              local.get 17
            else
              local.get 6
            end
            local.set 1
            local.get 7
            i32.const 1
            i32.eq
            br_if 0 (;@4;)
            local.get 1
            i32.const 2
            i32.shl
            local.set 13
            local.get 12
            local.get 1
            i32.const 240
            i32.mul
            i32.add
            local.set 0
            loop  ;; label = @5
              local.get 8
              local.get 2
              local.get 0
              i32.load
              local.get 10
              local.get 13
              i32.add
              local.tee 19
              i32.const 14160
              i32.add
              i32.load
              i32.add
              local.tee 5
              local.get 2
              local.get 5
              i32.gt_s
              select
              local.tee 2
              i32.store
              local.get 8
              local.get 2
              local.get 0
              i32.const 240
              i32.add
              i32.load
              local.get 19
              i32.const 14164
              i32.add
              i32.load
              i32.add
              local.tee 5
              local.get 2
              local.get 5
              i32.gt_s
              select
              local.tee 2
              i32.store
              local.get 0
              i32.const 480
              i32.add
              local.set 0
              local.get 13
              i32.const 8
              i32.add
              local.set 13
              local.get 1
              i32.const 2
              i32.add
              local.tee 1
              local.get 3
              i32.ne
              br_if 0 (;@5;)
            end
          end
          local.get 12
          i32.const 4
          i32.add
          local.set 12
          local.get 7
          i32.const 1
          i32.add
          local.set 7
          local.get 3
          i32.const 1
          i32.add
          local.tee 3
          i32.const 60
          i32.ne
          br_if 0 (;@3;)
        end
      end
      local.get 9
      i32.const 4
      i32.sub
      local.set 9
      local.get 10
      i32.const 240
      i32.sub
      local.set 10
      local.get 4
      i32.const 1
      i32.sub
      local.set 0
      local.get 4
      br_if 0 (;@1;)
    end)
  (memory (;0;) 31872 32768 (pagesize 1))
  (export "memory" (memory 0))
  (export "init" (func 0))
  (export "result" (func 1))
  (export "kernel" (func 2)))
