#ifndef NOVA_INPUT_H
#define NOVA_INPUT_H
/* Own /dev/kbd0 raw events until release or process termination. */
long nova_input_acquire(void);
long nova_input_release(void);
#endif
