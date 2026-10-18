#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <nova/fs.h>
typedef struct Block {struct Block *next;size_t size;int free;uint64_t _padding;} Block;
static Block *blocks;
static size_t aligned(size_t n){if(n>SIZE_MAX-15)return 0;return (n+15)&~(size_t)15;}
void *malloc(size_t size){
 if(size==0)size=1;size=aligned(size);if(!size)return NULL;
 for(Block *b=blocks;b;b=b->next){if(b->free&&b->size>=size){
  if(b->size>=size+sizeof(Block)+16){Block *tail=(Block*)((char*)(b+1)+size);tail->size=b->size-size-sizeof(Block);tail->free=1;tail->next=b->next;b->next=tail;b->size=size;}
  b->free=0;return b+1;
 }}
 if(size>SIZE_MAX-sizeof(Block)-4095)return NULL;
 size_t total=(size+sizeof(Block)+4095)&~(size_t)4095;if(total<65536)total=65536;
 Block *b=nova_map(total,3);if(!b)return NULL;
 b->next=blocks;b->size=total-sizeof(Block);b->free=1;blocks=b;
 return malloc(size);
}
void free(void *ptr){if(!ptr)return;Block *b=(Block*)ptr-1;b->free=1;}
void *calloc(size_t count,size_t size){if(size&&count>SIZE_MAX/size)return NULL;size_t n=count*size;void *p=malloc(n);if(p)memset(p,0,n);return p;}
void *realloc(void *ptr,size_t size){if(!ptr)return malloc(size);if(!size){free(ptr);return NULL;}Block *b=(Block*)ptr-1;if(b->size>=size)return ptr;void *p=malloc(size);if(!p)return NULL;memcpy(p,ptr,b->size);free(ptr);return p;}
int atoi(const char *s){return (int)strtol(s,NULL,10);}
long strtol(const char *s,char **end,int base){
 while(*s==' '||*s=='\t')s++;int negative=0;if(*s=='-'||*s=='+'){negative=*s=='-';s++;}
 if(base==0)base=10;if(base<2||base>36){if(end)*end=(char*)s;return 0;}
 long n=0;const char *p=s;for(;;p++){int d;if(*p>='0'&&*p<='9')d=*p-'0';else if(*p>='a'&&*p<='z')d=*p-'a'+10;else if(*p>='A'&&*p<='Z')d=*p-'A'+10;else break;if(d>=base)break;n=n*base+d;}
 if(end)*end=(char*)p;
 return negative?-n:n;
}
