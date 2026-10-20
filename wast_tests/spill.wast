;; own tests to check if register spills are managed correctly.
;; For i32 parameters, the first 16 are register-homed and the later ones stay on the frame.
;; For i64 parameters, the first 8 are register-homed and the later ones stay on the frame.
;; The hot parts of these expressions only use the frame-backed parameters.

(module
  (func $dummy)
  (func $dummy_i32 (param i32))
  (func $dummy_i64 (param i64))

  (func (export "i32_frame_right_spine")
    (param $a0 i32) (param $a1 i32) (param $a2 i32) (param $a3 i32)
    (param $a4 i32) (param $a5 i32) (param $a6 i32) (param $a7 i32)
    (param $a8 i32) (param $a9 i32) (param $a10 i32) (param $a11 i32)
    (param $a12 i32) (param $a13 i32) (param $a14 i32) (param $a15 i32)
    (param $a16 i32) (param $a17 i32) (param $a18 i32) (param $a19 i32)
    (param $a20 i32) (param $a21 i32) (param $a22 i32) (param $a23 i32)
    (param $a24 i32) (param $a25 i32) (param $a26 i32) (param $a27 i32)
    (param $a28 i32) (param $a29 i32) (param $a30 i32) (param $a31 i32)
    (result i32)
    (i32.add
      (i32.add (local.get $a16) (local.get $a17))
      (i32.add
        (i32.add (local.get $a18) (local.get $a19))
        (i32.add
          (i32.add (local.get $a20) (local.get $a21))
          (i32.add
            (i32.add (local.get $a22) (local.get $a23))
            (i32.add
              (i32.add (local.get $a24) (local.get $a25))
              (i32.add
                (i32.add (local.get $a26) (local.get $a27))
                (i32.add
                  (i32.add (local.get $a28) (local.get $a29))
                  (i32.add (local.get $a30) (local.get $a31)))))))))
    (call $dummy)
    (block
      (local.get $a0)
      (local.get $a1)
      (i32.add)
      (i32.const 3)
      (i32.mul)
      (call $dummy_i32)
      (if (i32.const 1)
        (then (call $dummy) )
        (else (call $dummy) )))
    
  )

  (func (export "i32_frame_tree")
    (param $a0 i32) (param $a1 i32) (param $a2 i32) (param $a3 i32)
    (param $a4 i32) (param $a5 i32) (param $a6 i32) (param $a7 i32)
    (param $a8 i32) (param $a9 i32) (param $a10 i32) (param $a11 i32)
    (param $a12 i32) (param $a13 i32) (param $a14 i32) (param $a15 i32)
    (param $a16 i32) (param $a17 i32) (param $a18 i32) (param $a19 i32)
    (param $a20 i32) (param $a21 i32) (param $a22 i32) (param $a23 i32)
    (param $a24 i32) (param $a25 i32) (param $a26 i32) (param $a27 i32)
    (param $a28 i32) (param $a29 i32) (param $a30 i32) (param $a31 i32)
    (result i32)
    (i32.add
      (i32.add
        (i32.add (local.get $a16) (local.get $a17))
        (i32.add (local.get $a18) (local.get $a19)))
      (i32.add
        (i32.add (local.get $a20) (local.get $a21))
        (i32.add
          (i32.add (local.get $a22) (local.get $a23))
          (i32.add
            (i32.add (local.get $a24) (local.get $a25))
            (i32.add
              (i32.add (local.get $a26) (local.get $a27))
              (i32.add
                (i32.add (local.get $a28) (local.get $a29))
                (i32.add (local.get $a30) (local.get $a31))))))))
    (call $dummy)
    (block
      (local.get $a2)
      (local.get $a3)
      (i32.sub)
      (local.get $a4)
      (i32.add)
      (call $dummy_i32)
      (if (i32.const 0)
        (then (call $dummy))
        (else (call $dummy))))
  )

  (func (export "i32_frame_select")
    (param $a0 i32) (param $a1 i32) (param $a2 i32) (param $a3 i32)
    (param $a4 i32) (param $a5 i32) (param $a6 i32) (param $a7 i32)
    (param $a8 i32) (param $a9 i32) (param $a10 i32) (param $a11 i32)
    (param $a12 i32) (param $a13 i32) (param $a14 i32) (param $a15 i32)
    (param $a16 i32) (param $a17 i32) (param $a18 i32) (param $a19 i32)
    (param $a20 i32) (param $a21 i32) (param $a22 i32) (param $a23 i32)
    (param $a24 i32) (param $a25 i32) (param $a26 i32) (param $a27 i32)
    (result i32)
    (select
      (i32.add
        (i32.add (local.get $a16) (local.get $a17))
        (i32.add (local.get $a18) (local.get $a19)))
      (i32.add
        (i32.add (local.get $a20) (local.get $a21))
        (i32.add (local.get $a22) (local.get $a23)))
      (i32.sub
        (i32.add (local.get $a24) (local.get $a25))
        (i32.add (local.get $a26) (local.get $a27))))
    (call $dummy)
    (block
      (local.get $a5)
      (local.get $a6)
      (i32.mul)
      (local.get $a7)
      (i32.add)
      (call $dummy_i32)
      (if (i32.const 1)
        (then (call $dummy) )
        (else (call $dummy) )))
    
  )

  (func (export "i64_frame_right_spine")
    (param $b0 i64) (param $b1 i64) (param $b2 i64) (param $b3 i64)
    (param $b4 i64) (param $b5 i64) (param $b6 i64) (param $b7 i64)
    (param $b8 i64) (param $b9 i64) (param $b10 i64) (param $b11 i64)
    (param $b12 i64) (param $b13 i64) (param $b14 i64) (param $b15 i64)
    (result i64)
    (i64.add
      (i64.add (local.get $b8) (local.get $b9))
      (i64.add
        (i64.add (local.get $b10) (local.get $b11))
        (i64.add
          (i64.add (local.get $b12) (local.get $b13))
          (i64.add (local.get $b14) (local.get $b15)))))
    (call $dummy)
    (block
      (local.get $b0)
      (local.get $b1)
      (i64.add)
      (local.get $b2)
      (i64.xor)
      (call $dummy_i64)
      (if (i32.const 1)
        (then (call $dummy) )
        (else (call $dummy) )))
    
  )

  (func (export "i64_frame_tree")
    (param $b0 i64) (param $b1 i64) (param $b2 i64) (param $b3 i64)
    (param $b4 i64) (param $b5 i64) (param $b6 i64) (param $b7 i64)
    (param $b8 i64) (param $b9 i64) (param $b10 i64) (param $b11 i64)
    (param $b12 i64) (param $b13 i64) (param $b14 i64) (param $b15 i64)
    (result i64)
    (i64.add
      (i64.add
        (i64.add (local.get $b8) (local.get $b9))
        (i64.add (local.get $b10) (local.get $b11)))
      (i64.add
        (i64.add (local.get $b12) (local.get $b13))
        (i64.add (local.get $b14) (local.get $b15))))
    (call $dummy)
    (block
      (local.get $b3)
      (local.get $b4)
      (i64.sub)
      (local.get $b5)
      (i64.add)
      (call $dummy_i64)
      (if (i32.const 0)
        (then (call $dummy) )
        (else (call $dummy) )))
    
  )

  (func (export "i64_frame_select")
    (param $b0 i64) (param $b1 i64) (param $b2 i64) (param $b3 i64)
    (param $b4 i64) (param $b5 i64) (param $b6 i64) (param $b7 i64)
    (param $b8 i64) (param $b9 i64) (param $b10 i64) (param $b11 i64)
    (param $b12 i64) (param $b13 i64) (param $b14 i64) (param $b15 i64)
    (result i64)
    (select
      (i64.add
        (i64.add (local.get $b8) (local.get $b9))
        (i64.add (local.get $b10) (local.get $b11)))
      (i64.add
        (i64.add (local.get $b12) (local.get $b13))
        (i64.add (local.get $b14) (local.get $b15)))
      (i64.eq
        (i64.add (local.get $b8) (local.get $b9))
        (i64.add (local.get $b10) (local.get $b11))))
    (call $dummy)
    (block
      (local.get $b6)
      (local.get $b7)
      (i64.mul)
      (local.get $b8)
      (i64.add)
      (call $dummy_i64)
      (if (i32.const 1)
        (then (call $dummy) )
        (else (call $dummy) )))
    
  )
)

(assert_return
  (invoke "i32_frame_right_spine"
    (i32.const 1) (i32.const 2) (i32.const 3) (i32.const 4)
    (i32.const 5) (i32.const 6) (i32.const 7) (i32.const 8)
    (i32.const 9) (i32.const 10) (i32.const 11) (i32.const 12)
    (i32.const 13) (i32.const 14) (i32.const 15) (i32.const 16)
    (i32.const 17) (i32.const 18) (i32.const 19) (i32.const 20)
    (i32.const 21) (i32.const 22) (i32.const 23) (i32.const 24)
    (i32.const 25) (i32.const 26) (i32.const 27) (i32.const 28)
    (i32.const 29) (i32.const 30) (i32.const 31) (i32.const 32))
  (i32.const 392))

(assert_return
  (invoke "i32_frame_tree"
    (i32.const 1) (i32.const 2) (i32.const 3) (i32.const 4)
    (i32.const 5) (i32.const 6) (i32.const 7) (i32.const 8)
    (i32.const 9) (i32.const 10) (i32.const 11) (i32.const 12)
    (i32.const 13) (i32.const 14) (i32.const 15) (i32.const 16)
    (i32.const 17) (i32.const 18) (i32.const 19) (i32.const 20)
    (i32.const 21) (i32.const 22) (i32.const 23) (i32.const 24)
    (i32.const 25) (i32.const 26) (i32.const 27) (i32.const 28)
    (i32.const 29) (i32.const 30) (i32.const 31) (i32.const 32))
  (i32.const 392))

(assert_return
  (invoke "i32_frame_select"
    (i32.const 1) (i32.const 2) (i32.const 3) (i32.const 4)
    (i32.const 5) (i32.const 6) (i32.const 7) (i32.const 8)
    (i32.const 9) (i32.const 10) (i32.const 11) (i32.const 12)
    (i32.const 13) (i32.const 14) (i32.const 15) (i32.const 16)
    (i32.const 17) (i32.const 18) (i32.const 19) (i32.const 20)
    (i32.const 21) (i32.const 22) (i32.const 23) (i32.const 24)
    (i32.const 25) (i32.const 26) (i32.const 27) (i32.const 28))
  (i32.const 74))

(assert_return
  (invoke "i32_frame_select"
    (i32.const 1) (i32.const 2) (i32.const 3) (i32.const 4)
    (i32.const 5) (i32.const 6) (i32.const 7) (i32.const 8)
    (i32.const 9) (i32.const 10) (i32.const 11) (i32.const 12)
    (i32.const 13) (i32.const 14) (i32.const 15) (i32.const 16)
    (i32.const 17) (i32.const 18) (i32.const 19) (i32.const 20)
    (i32.const 21) (i32.const 22) (i32.const 23) (i32.const 24)
    (i32.const 1) (i32.const 2) (i32.const 3) (i32.const 0))
  (i32.const 90))

(assert_return
  (invoke "i64_frame_right_spine"
    (i64.const 1) (i64.const 2) (i64.const 3) (i64.const 4)
    (i64.const 5) (i64.const 6) (i64.const 7) (i64.const 8)
    (i64.const 9) (i64.const 10) (i64.const 11) (i64.const 12)
    (i64.const 13) (i64.const 14) (i64.const 15) (i64.const 16))
  (i64.const 100))

(assert_return
  (invoke "i64_frame_tree"
    (i64.const 1) (i64.const 2) (i64.const 3) (i64.const 4)
    (i64.const 5) (i64.const 6) (i64.const 7) (i64.const 8)
    (i64.const 9) (i64.const 10) (i64.const 11) (i64.const 12)
    (i64.const 13) (i64.const 14) (i64.const 15) (i64.const 16))
  (i64.const 100))

(assert_return
  (invoke "i64_frame_select"
    (i64.const 1) (i64.const 2) (i64.const 3) (i64.const 4)
    (i64.const 5) (i64.const 6) (i64.const 7) (i64.const 8)
    (i64.const 4) (i64.const 5) (i64.const 3) (i64.const 6)
    (i64.const 10) (i64.const 20) (i64.const 30) (i64.const 40))
  (i64.const 18))

(assert_return
  (invoke "i64_frame_select"
    (i64.const 1) (i64.const 2) (i64.const 3) (i64.const 4)
    (i64.const 5) (i64.const 6) (i64.const 7) (i64.const 8)
    (i64.const 4) (i64.const 5) (i64.const 3) (i64.const 5)
    (i64.const 10) (i64.const 20) (i64.const 30) (i64.const 40))
  (i64.const 100))