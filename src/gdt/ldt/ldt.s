/*
 * TODO doc
 */

.global ldt_load

.section .text

/*
 * x86. Updates the TSS into the GDT.
 */
ldt_load:
	lldt 4(%esp)
	ret
