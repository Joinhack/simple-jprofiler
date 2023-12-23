int native_send_thread_signal(int thread_id, int signo) {
#ifdef __aarch64__
    register long x0 asm("x0") = thread_id;
    register long x1 asm("x1") = signo;
    register long x16 asm("x16") = 328;
    asm volatile("svc #0x80"
                 : "+r" (x0)
                 : "r" (x1), "r" (x16)
                 : "memory");
    return x0 == 0;
#else
    int result;
    asm volatile("syscall"
                 : "=a" (result)
                 : "a" (0x2000148), "D" (thread_id), "S" (signo)
                 : "rcx", "r11", "memory");
    return result == 0;
#endif
}