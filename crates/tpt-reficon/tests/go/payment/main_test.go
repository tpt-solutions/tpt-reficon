package main

import "testing"

// Mirrors the layout the Rust `#[ffi_contract]` test asserts from the header.
func TestPaymentLayoutMatchesHeader(t *testing.T) {
	size, align, offsets := layout()
	if size != 24 {
		t.Errorf("size = %d, want 24", size)
	}
	if align != 8 {
		t.Errorf("align = %d, want 8", align)
	}
	want := [4]uintptr{0, 8, 12, 16}
	if offsets != want {
		t.Errorf("offsets = %v, want %v", offsets, want)
	}
}
