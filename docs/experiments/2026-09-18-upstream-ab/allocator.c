#define _GNU_SOURCE
#include <dlfcn.h>
#include <malloc.h>
#include <stdlib.h>
#include <stdio.h>
int mallopt(int param, int value) {
 int (*real)(int,int)=dlsym(RTLD_NEXT,"mallopt");
 const char *v=getenv("AB_MMAP_THRESHOLD");
 if (param==M_MMAP_THRESHOLD && v) { value=atoi(v); fprintf(stderr,"AB mallopt mmap_threshold=%d\n",value); }
 return real(param,value);
}
