#include <pthread.h>
#include <stdio.h>
void *emptyfunc(void *dummy) { return 0; }
int main() {
  size_t i;
  pthread_attr_t attr;
  pthread_t td;
  pthread_attr_init(&attr);
  pthread_attr_setstacksize(&attr, 16384);
  for (i = 0; i < 2500; i++)
    pthread_create(&td, &attr, emptyfunc, 0);
  printf("Ok");
  return 0;
}
