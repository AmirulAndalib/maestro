#include <libc/string.h>

// TODO Rewrite
__attribute__((hot))
void *memmove(void *dest, const void *src, size_t n)
{
	size_t i;

	if(dest < src)
	{
		i = 0;

		while(i < n)
		{
			*((char *) dest + i) = *((char *) src + i);
			++i;
		}
	}
	else
	{
		i = n;

		do
		{
			*((char *) dest + (i - sizeof(char)))
				= *((char *) src + (i - sizeof(char)));
			i -= sizeof(char);
		}
		while(i != 0);
	}

	return dest;
}
