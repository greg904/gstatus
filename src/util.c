/*
 * Copyright (C) 2020 Greg Depoire--Ferrer <greg.depoire@gmail.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

#include "util.h"

bool util_fputs(int fd, const char *str, size_t len)
{
	return sys_write(fd, str, len) == (ssize_t)len;
}

void util_reverse(char *start, char *end)
{
	ASSERT(end >= start);

	while (start < end) {
		/* Swap bytes. */
		char tmp = *start;
		*start = *end;
		*end = tmp;

		start++;
		end--;
	}
}

void util_write_num(uint64_t num, char *str, size_t capacity)
{
	/* Write numbers in reverse order. */
	size_t i = 0;
	do {
		ASSERT(i < capacity);
		str[i] = '0' + num % 10;
		num /= 10;
		i++;
	} while (num != 0);

	/* Now, put the back in the correct order. */
	util_reverse(str, str + i - 1);

	/* Don't forget the NULL delimiter. */
	ASSERT(i < capacity);
	str[i] = '\0';
}

bool util_parse_num(const char *str, size_t len, uint64_t *out)
{
	*out = 0;

	for (size_t i = 0; i < len; ++i, ++str) {
		if (*str == '\n')
			break;

		if (*str < '0' || *str > '9')
			return false;

		*out *= 10;
		*out += *str - '0';
	}

	return true;
}

bool util_read_num_from_file(const char *path, uint64_t *out)
{
	int fd = sys_open(path, O_CLOEXEC, 0);
	if (fd < 0) {
		FPUTS_A(2, "open() failed\n");
		return false;
	}

	char buf[256];
	ssize_t bytes = sys_read(fd, buf, sizeof(buf));
	if (bytes < 0) {
		FPUTS_A(2, "read() failed\n");
		return false;
	}

	

	if (bytes == 0 || !util_parse_num(buf, bytes, out)) {
		FPUTS_A(2, "Failed to parse number.\n");
		return false;
	}

	return true;
}
