// SPDX-License-Identifier: GPL-3.0-or-later
//! Fechas del calendario gregoriano y segundos desde 1970, sin depender de la zona horaria.
//!
//! El historial guarda instantes como segundos desde 1970. El original escribe la fecha y
//! hora locales (`AAAA-MM-DD HH:MM`); al importarlas se toman tal cual, sin corregir la zona,
//! lo que alcanza para ordenar y graficar.

/// Días desde 1970-01-01 de una fecha (algoritmo de Howard Hinnant).
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Fecha `(año, mes, día)` de un día contado desde 1970-01-01.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    // `day` está en 1..=31 y `month` en 1..=12.
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

/// Lee `AAAA-MM-DD` y `HH:MM` (o `HH:MM:SS`) y devuelve segundos desde 1970.
pub fn parse_date_time(date: &str, time: &str) -> Option<i64> {
    let mut d = date.split('-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    if d.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut t = time.split(':');
    let hour: i64 = t.next()?.parse().ok()?;
    let minute: i64 = t.next()?.parse().ok()?;
    let second: i64 = t.next().map_or(Some(0), |s| s.parse().ok())?;
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) || !(0..60).contains(&second) {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second)
}

/// Formatea segundos desde 1970 como `AAAA-MM-DD HH:MM`.
pub fn format_date_time(secs: i64) -> String {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let rest = secs.rem_euclid(86_400);
    format!(
        "{y}-{m:02}-{d:02} {:02}:{:02}",
        rest / 3600,
        rest % 3600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(civil_from_days(11_017), (2000, 3, 1));
        for days in [-1000, 0, 59, 365, 10_957, 20_000, 30_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }

    #[test]
    fn parses_and_formats() {
        let t = parse_date_time("2021-03-04", "10:15").expect("fecha");
        assert_eq!(t, 1_614_852_900);
        assert_eq!(format_date_time(t), "2021-03-04 10:15");
        assert_eq!(parse_date_time("2021-13-01", "10:00"), None);
        assert_eq!(parse_date_time("2021-01-01", "25:00"), None);
        assert_eq!(parse_date_time("ayer", "10:00"), None);
    }
}
