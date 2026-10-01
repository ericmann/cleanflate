	.text
	.file	"sample.cgu"
	.file	1 "/x/crates/cleanflate/src" "fast_loop.rs"
	.file	2 "/x/crates/cleanflate/src" "window.rs"
	.section	.text._ZN10cleanflate9fast_loop3run17h0123456789abcdefE,"ax",@progbits
	.p2align	4
	.type	_ZN10cleanflate9fast_loop3run17h0123456789abcdefE,@function
_ZN10cleanflate9fast_loop3run17h0123456789abcdefE:
	.cfi_startproc
	.loc	1 10 5
	pushq	%rbp
	movq	%rdi, %rax
	cmpq	$4, %rsi
	jb	.LBB0_3
	.loc	2 77 9
	testq	%rdx, %rdx
	je	.LBB0_2
	jmp	.LBB0_4
.LBB0_2:
	leaq	.Lanon.aaaa.0(%rip), %rdx
	callq	*_ZN4core9panicking18panic_bounds_check17h1111111111111111E@GOTPCREL(%rip)
.LBB0_3:
	leaq	.Lanon.aaaa.1(%rip), %rdx
	callq	*_ZN4core5slice5index24slice_end_index_len_fail17h2222222222222222E@GOTPCREL(%rip)
.LBB0_4:
	leaq	.Lanon.aaaa.1(%rip), %rdx
	callq	*_ZN4core5slice5index24slice_end_index_len_fail17h2222222222222222E@GOTPCREL(%rip)
	callq	helper
	popq	%rbp
	retq
	.cfi_endproc
	.size	_ZN10cleanflate9fast_loop3run17h0123456789abcdefE, .-_ZN10cleanflate9fast_loop3run17h0123456789abcdefE

	.type	_ZN10cleanflate8checksum5crc3217h3333333333333333E,@function
_ZN10cleanflate8checksum5crc3217h3333333333333333E:
	.loc	1 99 1
	xorl	%eax, %eax
	testq	%rdi, %rdi
	jne	.LBB1_1
	retq
.LBB1_1:
	incq	%rax
	jne	.LBB1_1
	retq
	.size	_ZN10cleanflate8checksum5crc3217h3333333333333333E, .-_ZN10cleanflate8checksum5crc3217h3333333333333333E
