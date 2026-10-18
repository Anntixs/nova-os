#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <stdarg.h>
#include <nova/fs.h>
static FILE out={1},err={2};FILE *stdout=&out,*stderr=&err;
size_t fwrite(const void *p,size_t size,size_t count,FILE *stream){if(size&&count>((size_t)-1)/size)return 0;size_t n=size*count;long r=nova_write(stream->fd,p,n);return r<0?0:(size?((size_t)r/size):count);}
int putchar(int c){char b=(char)c;return nova_write(1,&b,1)==1?(unsigned char)b:-1;}
int puts(const char *s){size_t n=strlen(s);if(nova_write(1,s,n)!=(long)n||nova_write(1,"\n",1)!=1)return -1;return (int)(n+1);}
static int emit(const char *s,size_t n){return nova_write(1,s,n)==(long)n?(int)n:-1;}
static int number(uint64_t n,unsigned base,int negative,int upper){char buf[32];size_t i=sizeof(buf);do{unsigned d=(unsigned)(n%base);buf[--i]=(char)(d<10?'0'+d:(upper?'A':'a')+d-10);n/=base;}while(n);if(negative)buf[--i]='-';return emit(buf+i,sizeof(buf)-i);}
int printf(const char *fmt,...){va_list ap;va_start(ap,fmt);int total=0;
 for(const char *p=fmt;*p;p++){if(*p!='%'){int n=emit(p,1);if(n<0){total=-1;break;}total+=n;continue;}
  p++;if(!*p)break;int n=0;
  switch(*p){case '%':n=emit("%",1);break;case 'c':{char c=(char)va_arg(ap,int);n=emit(&c,1);break;}
   case 's':{const char *s=va_arg(ap,const char*);if(!s)s="(null)";n=emit(s,strlen(s));break;}
   case 'd':{int value=va_arg(ap,int);uint64_t mag=value<0?(uint64_t)(-(int64_t)value):(uint64_t)value;n=number(mag,10,value<0,0);break;}
   case 'u':n=number(va_arg(ap,unsigned),10,0,0);break;
   case 'x':n=number(va_arg(ap,unsigned),16,0,0);break;
   case 'p':{void *value=va_arg(ap,void*);n=emit("0x",2);if(n>=0){int k=number((uintptr_t)value,16,0,0);n=k<0?-1:n+k;}break;}
   default:n=-1;break;}
  if(n<0){total=-1;break;}total+=n;
 }
 va_end(ap);return total;}
