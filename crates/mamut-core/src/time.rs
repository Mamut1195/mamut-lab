//! Tiempo inyectable.
//!
//! El dominio **no** lee el reloj del sistema: recibe un [`Timestamp`] o un [`Clock`].
//! Sin esta separación, cada prueba dependería de la hora real y dejaría de ser
//! reproducible (ADR-0007).
//!
//! Se distinguen dos conceptos, y la distinción es deliberada:
//! - [`Timestamp`]: un **instante** (auditoría, cálculo, emisión). Siempre en UTC.
//! - [`CivilDate`]: una **fecha del calendario** (fecha de recepción, próxima
//!   calibración). Sin hora ni zona: una muestra recibida el 18 de noviembre lo fue
//!   ese día, no «a las 23:59 UTC».

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, Result};

// ------------------------------------------------------------------ reloj ---

/// Fuente de tiempo. En producción [`SystemClock`]; en pruebas [`FixedClock`].
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

/// Reloj real. Única lectura del reloj del sistema en todo el dominio.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp(Utc::now().timestamp_millis())
    }
}

/// Reloj fijo, para pruebas deterministas.
#[derive(Clone, Copy, Debug)]
pub struct FixedClock(Timestamp);

impl FixedClock {
    pub fn new(at: Timestamp) -> Self {
        Self(at)
    }

    /// Reloj detenido en una fecha civil, a medianoche UTC.
    pub fn at_date(date: CivilDate) -> Self {
        Self(date.to_timestamp_midnight_utc())
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}

// -------------------------------------------------------------- instante ---

/// Un instante, en milisegundos desde la época Unix, siempre UTC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    pub fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    pub fn millis(self) -> i64 {
        self.0
    }

    /// Texto ISO-8601 en UTC, con milisegundos: `2026-11-18T20:31:07.123Z`.
    pub fn to_rfc3339(self) -> String {
        DateTime::<Utc>::from_timestamp_millis(self.0)
            .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
            .unwrap_or_else(|| format!("{}+00:00", self.0))
    }

    pub fn parse_rfc3339(text: &str) -> Result<Self> {
        DateTime::parse_from_rfc3339(text)
            .map(|d| Self(d.timestamp_millis()))
            .map_err(|_| Error::InvalidDate(text.to_owned()))
    }

    /// Fecha civil en UTC a la que pertenece el instante.
    pub fn utc_date(self) -> CivilDate {
        match DateTime::<Utc>::from_timestamp_millis(self.0) {
            Some(d) => CivilDate(d.date_naive()),
            // Un instante fuera del rango representable no pertenece a ninguna fecha:
            // se devuelve el mínimo en lugar de inventar una.
            None => CivilDate(NaiveDate::MIN),
        }
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_rfc3339())
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse_rfc3339(&text).map_err(serde::de::Error::custom)
    }
}

// ----------------------------------------------------------- fecha civil ---

/// Fecha del calendario, sin hora ni zona horaria.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDate(NaiveDate);

impl CivilDate {
    /// Fecha a partir de año, mes y día. Rechaza fechas inexistentes (31 de febrero).
    pub fn new(year: i32, month: u32, day: u32) -> Result<Self> {
        NaiveDate::from_ymd_opt(year, month, day)
            .map(Self)
            .ok_or_else(|| Error::InvalidDate(format!("{year:04}-{month:02}-{day:02}")))
    }

    /// Parseo estricto `AAAA-MM-DD`.
    pub fn parse(text: &str) -> Result<Self> {
        // `NaiveDate::parse_from_str` con este formato acepta también `2026-1-1`;
        // se exige la forma canónica para que la entrada sea inequívoca.
        if text.len() != 10 || text.as_bytes()[4] != b'-' || text.as_bytes()[7] != b'-' {
            return Err(Error::InvalidDate(text.to_owned()));
        }
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .map(Self)
            .map_err(|_| Error::InvalidDate(text.to_owned()))
    }

    pub fn to_iso(self) -> String {
        self.0.format("%Y-%m-%d").to_string()
    }

    pub fn year(self) -> i32 {
        self.0.year()
    }

    /// La fecha de hoy según el reloj inyectado.
    pub fn today(clock: &dyn Clock) -> Self {
        clock.now().utc_date()
    }

    /// Suma (o resta, con signo) días naturales.
    pub fn add_days(self, days: i64) -> Self {
        let delta = chrono::Days::new(days.unsigned_abs());
        Self(if days >= 0 {
            self.0 + delta
        } else {
            self.0 - delta
        })
    }

    /// Días de `self` a `later` (negativo si `later` es anterior).
    pub fn days_until(self, later: CivilDate) -> i64 {
        (later.0 - self.0).num_days()
    }

    /// Medianoche UTC de esa fecha, como instante.
    pub fn to_timestamp_midnight_utc(self) -> Timestamp {
        match self.0.and_hms_opt(0, 0, 0) {
            Some(dt) => Timestamp(dt.and_utc().timestamp_millis()),
            None => Timestamp(0),
        }
    }
}

impl std::fmt::Display for CivilDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_iso())
    }
}

impl Serialize for CivilDate {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_iso())
    }
}

impl<'de> Deserialize<'de> for CivilDate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fecha_ida_y_vuelta() {
        let d = CivilDate::new(2026, 11, 18).unwrap();
        assert_eq!(d.to_iso(), "2026-11-18");
        assert_eq!(CivilDate::parse("2026-11-18").unwrap(), d);
    }

    #[test]
    fn rechaza_fechas_inexistentes() {
        assert!(CivilDate::new(2026, 2, 31).is_err());
        assert!(CivilDate::new(2026, 13, 1).is_err());
        assert!(CivilDate::new(2026, 0, 10).is_err());
    }

    #[test]
    fn acepta_el_29_de_febrero_en_bisiesto_y_lo_rechaza_fuera() {
        assert!(CivilDate::new(2024, 2, 29).is_ok());
        assert!(CivilDate::new(2025, 2, 29).is_err());
    }

    #[test]
    fn el_parseo_exige_la_forma_canonica() {
        assert!(CivilDate::parse("2026-1-1").is_err());
        assert!(CivilDate::parse("18/11/2026").is_err());
        assert!(CivilDate::parse("2026-11-18T00:00:00Z").is_err());
        assert!(CivilDate::parse("2026-11-18").is_ok());
    }

    #[test]
    fn diferencia_de_dias_con_signo() {
        let a = CivilDate::new(2026, 11, 18).unwrap();
        let b = CivilDate::new(2026, 12, 18).unwrap();
        assert_eq!(a.days_until(b), 30);
        assert_eq!(b.days_until(a), -30);
        assert_eq!(a.days_until(a), 0);
    }

    #[test]
    fn sumar_dias_cruza_fin_de_mes() {
        let a = CivilDate::new(2026, 11, 18).unwrap();
        assert_eq!(a.add_days(20).to_iso(), "2026-12-08");
        assert_eq!(a.add_days(-18).to_iso(), "2026-10-31");
    }

    #[test]
    fn el_reloj_fijo_es_determinista() {
        let date = CivilDate::new(2026, 11, 18).unwrap();
        let clock = FixedClock::at_date(date);
        assert_eq!(CivilDate::today(&clock), date);
        assert_eq!(clock.now(), clock.now());
    }

    #[test]
    fn instante_ida_y_vuelta_en_texto() {
        let ts = Timestamp::from_millis(1_763_500_000_000);
        let texto = ts.to_rfc3339();
        assert!(texto.ends_with('Z'), "{texto}");
        assert_eq!(Timestamp::parse_rfc3339(&texto).unwrap(), ts);
    }

    #[test]
    fn instante_invalido_se_rechaza() {
        assert!(Timestamp::parse_rfc3339("ayer").is_err());
    }

    #[test]
    fn la_fecha_civil_se_serializa_como_texto_iso() {
        let d = CivilDate::new(2026, 11, 18).unwrap();
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, "\"2026-11-18\"");
        let vuelta: CivilDate = serde_json::from_str(&json).unwrap();
        assert_eq!(d, vuelta);
    }

    #[test]
    fn una_fecha_invalida_no_entra_por_deserializacion() {
        assert!(serde_json::from_str::<CivilDate>("\"2026-02-31\"").is_err());
    }
}
