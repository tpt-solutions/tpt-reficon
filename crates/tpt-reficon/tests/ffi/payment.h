/* Sample C header for the #[ffi_contract] integration tests. */
#include <stdint.h>

struct payment {
    uint64_t id;
    int32_t amount;
    uint8_t flag;
    double *ptr;
};

/* Sample function for the ffi_wrappers! tests. */
int32_t process_payment(int32_t amount);
