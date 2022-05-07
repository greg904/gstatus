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

#include <limits.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdnoreturn.h>

#include <flibc/linux.h>
#include <flibc/util.h>

static uint32_t battery_read_interval = 60;
static uint32_t timezone = 2;

static void write_time_component(time_t val, char *buf);

int main(int argc, char **argv)
{
	F_UNUSED(argc);
	F_UNUSED(argv);

	if (!F_PRINT(1, "{\"version\":1}\n["))
		return 1;

	time_t last_battery_read = 0;
	uint64_t energy_now = UINT64_MAX;
	uint64_t energy_full = UINT64_MAX;

	for (;;) {
		struct timespec ts_monotonic;
		if (sys_clock_gettime(CLOCK_MONOTONIC, &ts_monotonic) != 0) {
			F_PRINT(2, "clock_gettime() failed\n");
			return 1;
		}

		if (last_battery_read == 0 ||
		    ts_monotonic.tv_sec - last_battery_read >=
			battery_read_interval) {
			// Read battery level.
			if (!util_read_num_from_file(
				"/sys/class/power_supply/BAT0/energy_now",
				&energy_now) ||
			    !util_read_num_from_file(
				"/sys/class/power_supply/BAT0/energy_full",
				&energy_full)) {
				energy_now = UINT64_MAX;
				energy_full = UINT64_MAX;
			}
			last_battery_read = ts_monotonic.tv_sec;
		}

		struct timespec ts_realtime;
		if (sys_clock_gettime(CLOCK_REALTIME, &ts_realtime) != 0) {
			F_PRINT(2, "clock_gettime()\n");
			return 1;
		}

		if (!F_PRINT(1, "["))
			return 1;

		if (energy_now != UINT64_MAX && energy_full != UINT64_MAX) {
			char tmp[21];
			util_write_num(energy_now * 100 / energy_full, tmp, sizeof(tmp) / sizeof(*tmp));

			if (!F_PRINT(1, "{\"full_text\":\"Battery: ") ||
				!F_PRINT(1, tmp) ||
				!F_PRINT(1, "%\"},"))
				return 1;
		}

		time_t total_minutes = ts_realtime.tv_sec / 60;

		time_t hours = ((total_minutes / 60) + timezone) % 24;
		char hours_buf[3];
		write_time_component(hours, hours_buf);

		time_t minutes = total_minutes % 60;
		char minutes_buf[3];
		write_time_component(minutes, minutes_buf);

		if (!F_PRINT(1, "{\"full_text\":\"") ||
			!F_PRINT(1, hours_buf) ||
			!F_PRINT(1, ":") ||
			!F_PRINT(1, minutes_buf) ||
			!F_PRINT(1, "\"}],"))
			return 1;

		int sleep_s = INT_MAX;

		// Sleep until at most the next minute.
		int next_minute_s = 61 - (ts_realtime.tv_sec % 60);
		if (next_minute_s < sleep_s)
			sleep_s = next_minute_s;

		// Make sure not to miss the battery poll interval.
		int next_battery_read_s =
		    battery_read_interval -
		    (ts_monotonic.tv_sec - last_battery_read);
		if (next_battery_read_s < sleep_s)
			sleep_s = next_battery_read_s;

		struct timespec ts_sleep;
		ts_sleep.tv_sec = sleep_s;
		ts_sleep.tv_nsec = 0;

		while (ts_sleep.tv_sec > 0) {
			int ret = sys_nanosleep(&ts_sleep, &ts_sleep);
			if (ret == 0) {
				/* Sleeping has completed. */
				break;
			} else if (ret == -EINTR) {
				/* We were interupted... Go back to
				   sleeping. */
				continue;
			} else {
				F_PRINT(2, "nanosleep() failed");
				return 1;
			}
		}
	}
}

static void write_time_component(time_t val, char *buf)
{
	if (val >= 10) {
		F_ASSERT(val / 10 < 10);
		buf[0] = '0' + (val / 10);
		buf[1] = '0' + val % 10;
	} else {
		buf[0] = '0';
		buf[1] = '0' + val % 10;
	}
	buf[2] = '\0';
}
