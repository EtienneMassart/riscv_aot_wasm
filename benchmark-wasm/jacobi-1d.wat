(module
  (type (;0;) (func))
  (type (;1;) (func (result i64)))
  (func (;0;) (type 0)
    (local i32 i32 f32)
    loop  ;; label = @1
      local.get 0
      i32.const 1152
      i32.add
      local.get 1
      f32.convert_i32_u
      local.tee 2
      f32.const 0x1.8p+1 (;=3;)
      f32.add
      f32.const 0x1.ep+4 (;=30;)
      f32.div
      f32.store
      local.get 0
      i32.const 1024
      i32.add
      local.get 2
      f32.const 0x1p+1 (;=2;)
      f32.add
      f32.const 0x1.ep+4 (;=30;)
      f32.div
      f32.store
      local.get 0
      i32.const 4
      i32.add
      local.set 0
      local.get 1
      i32.const 1
      i32.add
      local.tee 1
      i32.const 30
      i32.ne
      br_if 0 (;@1;)
    end)
  (func (;1;) (type 1) (result i64)
    (local f32 i32)
    i32.const 17648
    local.tee 1
    i64.const 0
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1024
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1028
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1032
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1036
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1040
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1044
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1048
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1052
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1056
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1060
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1064
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1068
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1072
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1076
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1080
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1084
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1088
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1092
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1096
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1100
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1104
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1108
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1112
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1116
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1120
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1124
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1128
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1132
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1136
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    block (result i64)  ;; label = @1
      i32.const 1140
      f32.load
      local.tee 0
      f32.const 0x1p+64 (;=1.84467e+19;)
      f32.lt
      local.get 0
      f32.const 0x0p+0 (;=0;)
      f32.ge
      i32.and
      if  ;; label = @2
        local.get 0
        i64.trunc_f32_u
        br 1 (;@1;)
      end
      i64.const 0
    end
    local.get 1
    i64.load offset=8
    i64.add
    i64.store offset=8
    local.get 1
    i64.load offset=8)
  (func (;2;) (type 0)
    (local f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 f32 i32)
    i32.const 1268
    f32.load
    local.set 56
    i32.const 1136
    f32.load
    local.set 26
    i32.const 1132
    f32.load
    local.set 0
    i32.const 1128
    f32.load
    local.set 1
    i32.const 1124
    f32.load
    local.set 2
    i32.const 1120
    f32.load
    local.set 3
    i32.const 1116
    f32.load
    local.set 4
    i32.const 1108
    f32.load
    local.set 5
    i32.const 1112
    f32.load
    local.set 6
    i32.const 1104
    f32.load
    local.set 7
    i32.const 1072
    f32.load
    local.set 8
    i32.const 1068
    f32.load
    local.set 9
    i32.const 1064
    f32.load
    local.set 10
    i32.const 1060
    f32.load
    local.set 11
    i32.const 1056
    f32.load
    local.set 12
    i32.const 1052
    f32.load
    local.set 13
    i32.const 1048
    f32.load
    local.set 14
    i32.const 1044
    f32.load
    local.set 15
    i32.const 1040
    f32.load
    local.set 16
    i32.const 1036
    f32.load
    local.set 17
    i32.const 1032
    f32.load
    local.set 18
    i32.const 1028
    f32.load
    local.set 27
    i32.const 1152
    f32.load
    local.set 57
    i32.const 1140
    f32.load
    local.set 58
    i32.const 1024
    f32.load
    local.set 59
    i32.const 1100
    f32.load
    local.set 19
    i32.const 1096
    f32.load
    local.set 20
    i32.const 1092
    f32.load
    local.set 21
    i32.const 1088
    f32.load
    local.set 22
    i32.const 1084
    f32.load
    local.set 23
    i32.const 1080
    f32.load
    local.set 24
    i32.const 1076
    f32.load
    local.set 25
    i32.const 20
    local.set 60
    loop  ;; label = @1
      local.get 26
      local.get 0
      f32.add
      local.get 58
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 54
      local.get 0
      local.get 1
      f32.add
      local.get 26
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 28
      f32.add
      local.get 56
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 26
      local.get 28
      local.get 1
      local.get 2
      f32.add
      local.get 0
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 29
      f32.add
      local.get 54
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 0
      local.get 29
      local.get 2
      local.get 3
      f32.add
      local.get 1
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 30
      f32.add
      local.get 28
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 1
      local.get 30
      local.get 3
      local.get 4
      f32.add
      local.get 2
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 31
      f32.add
      local.get 29
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 2
      local.get 31
      local.get 4
      local.get 6
      f32.add
      local.get 3
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 32
      f32.add
      local.get 30
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 3
      local.get 32
      local.get 6
      local.get 5
      f32.add
      local.get 4
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 33
      f32.add
      local.get 31
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 4
      local.get 33
      local.get 5
      local.get 7
      f32.add
      local.get 6
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 34
      f32.add
      local.get 32
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 6
      local.get 34
      local.get 7
      local.get 19
      f32.add
      local.get 5
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 35
      f32.add
      local.get 33
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 5
      local.get 35
      local.get 19
      local.get 20
      f32.add
      local.get 7
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 36
      f32.add
      local.get 34
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 7
      local.get 36
      local.get 20
      local.get 21
      f32.add
      local.get 19
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 37
      f32.add
      local.get 35
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 19
      local.get 37
      local.get 21
      local.get 22
      f32.add
      local.get 20
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 38
      f32.add
      local.get 36
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 20
      local.get 38
      local.get 22
      local.get 23
      f32.add
      local.get 21
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 39
      f32.add
      local.get 37
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 21
      local.get 39
      local.get 23
      local.get 24
      f32.add
      local.get 22
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 40
      f32.add
      local.get 38
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 22
      local.get 40
      local.get 24
      local.get 25
      f32.add
      local.get 23
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 41
      f32.add
      local.get 39
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 23
      local.get 41
      local.get 25
      local.get 8
      f32.add
      local.get 24
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 42
      f32.add
      local.get 40
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 24
      local.get 42
      local.get 8
      local.get 9
      f32.add
      local.get 25
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 43
      f32.add
      local.get 41
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 25
      local.get 43
      local.get 9
      local.get 10
      f32.add
      local.get 8
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 44
      f32.add
      local.get 42
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 8
      local.get 44
      local.get 10
      local.get 11
      f32.add
      local.get 9
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 45
      f32.add
      local.get 43
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 9
      local.get 45
      local.get 11
      local.get 12
      f32.add
      local.get 10
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 46
      f32.add
      local.get 44
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 10
      local.get 46
      local.get 12
      local.get 13
      f32.add
      local.get 11
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 47
      f32.add
      local.get 45
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 11
      local.get 47
      local.get 13
      local.get 14
      f32.add
      local.get 12
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 48
      f32.add
      local.get 46
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 12
      local.get 48
      local.get 14
      local.get 15
      f32.add
      local.get 13
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 49
      f32.add
      local.get 47
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 13
      local.get 49
      local.get 15
      local.get 16
      f32.add
      local.get 14
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 50
      f32.add
      local.get 48
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 14
      local.get 50
      local.get 16
      local.get 17
      f32.add
      local.get 15
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 51
      f32.add
      local.get 49
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 15
      local.get 51
      local.get 17
      local.get 18
      f32.add
      local.get 16
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 52
      f32.add
      local.get 50
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 16
      local.get 52
      local.get 18
      local.get 27
      f32.add
      local.get 17
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 53
      f32.add
      local.get 51
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 17
      local.get 53
      local.get 27
      local.get 59
      f32.add
      local.get 18
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.tee 55
      f32.add
      local.get 52
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 18
      local.get 55
      local.get 57
      f32.add
      local.get 53
      f32.add
      f32.const 0x1.555476p-2 (;=0.33333;)
      f32.mul
      local.set 27
      local.get 60
      i32.const 1
      i32.sub
      local.tee 60
      br_if 0 (;@1;)
    end
    i32.const 1032
    local.get 18
    f32.store
    i32.const 1028
    local.get 27
    f32.store
    i32.const 1156
    local.get 55
    f32.store
    i32.const 1036
    local.get 17
    f32.store
    i32.const 1160
    local.get 53
    f32.store
    i32.const 1040
    local.get 16
    f32.store
    i32.const 1164
    local.get 52
    f32.store
    i32.const 1044
    local.get 15
    f32.store
    i32.const 1168
    local.get 51
    f32.store
    i32.const 1048
    local.get 14
    f32.store
    i32.const 1172
    local.get 50
    f32.store
    i32.const 1052
    local.get 13
    f32.store
    i32.const 1176
    local.get 49
    f32.store
    i32.const 1056
    local.get 12
    f32.store
    i32.const 1180
    local.get 48
    f32.store
    i32.const 1060
    local.get 11
    f32.store
    i32.const 1184
    local.get 47
    f32.store
    i32.const 1064
    local.get 10
    f32.store
    i32.const 1188
    local.get 46
    f32.store
    i32.const 1192
    local.get 45
    f32.store
    i32.const 1068
    local.get 9
    f32.store
    i32.const 1072
    local.get 8
    f32.store
    i32.const 1232
    local.get 35
    f32.store
    i32.const 1228
    local.get 36
    f32.store
    i32.const 1224
    local.get 37
    f32.store
    i32.const 1220
    local.get 38
    f32.store
    i32.const 1216
    local.get 39
    f32.store
    i32.const 1212
    local.get 40
    f32.store
    i32.const 1208
    local.get 41
    f32.store
    i32.const 1204
    local.get 42
    f32.store
    i32.const 1200
    local.get 43
    f32.store
    i32.const 1196
    local.get 44
    f32.store
    i32.const 1104
    local.get 7
    f32.store
    i32.const 1112
    local.get 6
    f32.store
    i32.const 1236
    local.get 34
    f32.store
    i32.const 1116
    local.get 4
    f32.store
    i32.const 1108
    local.get 5
    f32.store
    i32.const 1240
    local.get 33
    f32.store
    i32.const 1120
    local.get 3
    f32.store
    i32.const 1244
    local.get 32
    f32.store
    i32.const 1124
    local.get 2
    f32.store
    i32.const 1248
    local.get 31
    f32.store
    i32.const 1128
    local.get 1
    f32.store
    i32.const 1252
    local.get 30
    f32.store
    i32.const 1132
    local.get 0
    f32.store
    i32.const 1256
    local.get 29
    f32.store
    i32.const 1136
    local.get 26
    f32.store
    i32.const 1264
    local.get 54
    f32.store
    i32.const 1260
    local.get 28
    f32.store
    i32.const 1100
    local.get 19
    f32.store
    i32.const 1096
    local.get 20
    f32.store
    i32.const 1092
    local.get 21
    f32.store
    i32.const 1088
    local.get 22
    f32.store
    i32.const 1084
    local.get 23
    f32.store
    i32.const 1080
    local.get 24
    f32.store
    i32.const 1076
    local.get 25
    f32.store)
  (memory (;0;) 17664 32768 (pagesize 1))
  (export "memory" (memory 0))
  (export "init" (func 0))
  (export "result" (func 1))
  (export "kernel" (func 2)))
