// Package main is the Go side of the `#[ffi_contract(bridge = "go")]` test.
//
// Build the C header that the Rust test reads with:
//
//	go build -buildmode=c-shared -o payment.dll .
//
// The generated payment.h is checked in as go/payment/payment.h, so the Rust
// test does not need Go installed. Run `go test` here to check that Go's own
// view of the struct matches the header.
package main

/*
#include <stdint.h>

struct payment {
    uint64_t id;
    int32_t amount;
    uint8_t flag;
    double *ptr;
};

static int payment_amount(const struct payment *p) { return p->amount; }
*/
import "C"

import "unsafe"

// ProcessPayment is exported so the struct appears in the generated header.
//
//export ProcessPayment
func ProcessPayment(p *C.struct_payment) C.int32_t {
	return C.payment_amount(p)
}

// layout reports Go's own view of struct payment: size, alignment, and the
// offsets of id, amount, flag, and ptr, in that order.
func layout() (size, align uintptr, offsets [4]uintptr) {
	var p C.struct_payment
	return unsafe.Sizeof(p), unsafe.Alignof(p), [4]uintptr{
		unsafe.Offsetof(p.id),
		unsafe.Offsetof(p.amount),
		unsafe.Offsetof(p.flag),
		unsafe.Offsetof(p.ptr),
	}
}

func main() {}
