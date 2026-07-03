#include "agbot_worldgen/Crs.hpp"

#include <algorithm>
#include <cctype>
#include <cmath>

namespace agbot::worldgen {

namespace {

constexpr double kPi = 3.14159265358979323846;
constexpr double kDeg2Rad = kPi / 180.0;
constexpr double kRad2Deg = 180.0 / kPi;

// GRS80 ellipsoid (NAD83).
constexpr double kA = 6378137.0;                     // semi-major axis (m)
constexpr double kF = 1.0 / 298.257222101;           // flattening
const double kE2 = kF * (2.0 - kF);                  // first eccentricity squared
const double kE = std::sqrt(kE2);

// 1 US survey foot in metres (exactly 1200/3937).
constexpr double kUsFootM = 1200.0 / 3937.0;

std::string to_lower(std::string value) {
    std::transform(value.begin(), value.end(), value.begin(),
                   [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return value;
}

// Lambert Conformal Conic (2SP) constants for EPSG:2263, precomputed once.
struct LccNyLi {
    double n = 0.0;
    double big_f = 0.0;
    double rho0 = 0.0;
    double lambda0 = 0.0;   // central meridian (rad)
    double e0_m = 0.0;      // false easting (m)
    double n0_m = 0.0;      // false northing (m)

    LccNyLi() {
        const double phi1 = (41.0 + 2.0 / 60.0) * kDeg2Rad;   // 41°02'
        const double phi2 = (40.0 + 40.0 / 60.0) * kDeg2Rad;  // 40°40'
        const double phi0 = (40.0 + 10.0 / 60.0) * kDeg2Rad;  // 40°10'
        lambda0 = -74.0 * kDeg2Rad;
        e0_m = 300000.0;   // 984250 US ft
        n0_m = 0.0;

        const double m1 = m_of(phi1);
        const double m2 = m_of(phi2);
        const double t0 = t_of(phi0);
        const double t1 = t_of(phi1);
        const double t2 = t_of(phi2);
        n = (std::log(m1) - std::log(m2)) / (std::log(t1) - std::log(t2));
        big_f = m1 / (n * std::pow(t1, n));
        rho0 = kA * big_f * std::pow(t0, n);
    }

    static double m_of(double phi) {
        return std::cos(phi) / std::sqrt(1.0 - kE2 * std::sin(phi) * std::sin(phi));
    }
    static double t_of(double phi) {
        const double sin_phi = std::sin(phi);
        return std::tan(kPi / 4.0 - phi / 2.0) /
            std::pow((1.0 - kE * sin_phi) / (1.0 + kE * sin_phi), kE / 2.0);
    }
};

const LccNyLi& lcc() {
    static const LccNyLi instance;
    return instance;
}

// Transverse Mercator constants for EPSG:26918 (UTM zone 18N).
constexpr double kUtmK0 = 0.9996;
constexpr double kUtmE0 = 500000.0;
constexpr double kUtmN0 = 0.0;
const double kUtmLambda0 = -75.0 * kDeg2Rad;

// Meridian arc length from the equator to latitude phi (GRS80).
double meridian_arc(double phi) {
    const double e4 = kE2 * kE2;
    const double e6 = e4 * kE2;
    return kA * ((1.0 - kE2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * phi -
                 (3.0 * kE2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * std::sin(2.0 * phi) +
                 (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * std::sin(4.0 * phi) -
                 (35.0 * e6 / 3072.0) * std::sin(6.0 * phi));
}

} // namespace

HorizontalCrs horizontal_crs_from_epsg(const std::string& code) {
    std::string normalized = to_lower(code);
    if (normalized.rfind("epsg:", 0) == 0) {
        normalized = normalized.substr(5);
    }
    if (normalized.empty() || normalized == "4326" || normalized == "wgs84") {
        return HorizontalCrs::Wgs84Lonlat;
    }
    if (normalized == "2263") {
        return HorizontalCrs::StatePlaneNyLongIslandFt;
    }
    if (normalized == "26918" || normalized == "utm18n") {
        return HorizontalCrs::Utm18N;
    }
    return HorizontalCrs::Unknown;
}

const char* epsg_for(HorizontalCrs crs) {
    switch (crs) {
        case HorizontalCrs::Wgs84Lonlat: return "EPSG:4326";
        case HorizontalCrs::StatePlaneNyLongIslandFt: return "EPSG:2263";
        case HorizontalCrs::Utm18N: return "EPSG:26918";
        case HorizontalCrs::Unknown: return "EPSG:4326";
    }
    return "EPSG:4326";
}

VerticalDatum vertical_datum_from_name(const std::string& name) {
    const std::string n = to_lower(name);
    if (n.empty()) {
        return VerticalDatum::Unknown;
    }
    if (n == "none") {
        return VerticalDatum::None;
    }
    if (n == "navd88") {
        return VerticalDatum::Navd88;
    }
    if (n == "navd88_geoid18" || n == "navd88-geoid18" || n == "geoid18") {
        return VerticalDatum::Navd88Geoid18;
    }
    if (n == "ellipsoidal" || n == "nad83" || n == "ellipsoid") {
        return VerticalDatum::Ellipsoidal;
    }
    return VerticalDatum::Unknown;
}

const char* to_string(VerticalDatum datum) {
    switch (datum) {
        case VerticalDatum::Unknown: return "unknown";
        case VerticalDatum::None: return "none";
        case VerticalDatum::Navd88: return "NAVD88";
        case VerticalDatum::Navd88Geoid18: return "NAVD88_GEOID18";
        case VerticalDatum::Ellipsoidal: return "ellipsoidal";
    }
    return "unknown";
}

bool vertical_datums_compatible(VerticalDatum a, VerticalDatum b) {
    if (a == VerticalDatum::Unknown || b == VerticalDatum::Unknown ||
        a == VerticalDatum::None || b == VerticalDatum::None) {
        return true;
    }
    if (a == b) {
        return true;
    }
    const auto navd88_family = [](VerticalDatum d) {
        return d == VerticalDatum::Navd88 || d == VerticalDatum::Navd88Geoid18;
    };
    return navd88_family(a) && navd88_family(b);
}

GeoCoordinate wgs84_from_state_plane_li_ft(double easting_ft, double northing_ft) {
    const LccNyLi& c = lcc();
    const double easting = easting_ft * kUsFootM;
    const double northing = northing_ft * kUsFootM;

    const double de = easting - c.e0_m;
    const double dn = c.rho0 - (northing - c.n0_m);
    const double sign_n = c.n > 0.0 ? 1.0 : -1.0;
    const double rho = sign_n * std::sqrt(de * de + dn * dn);
    const double theta = std::atan2(de, dn);

    const double t = std::pow(rho / (kA * c.big_f), 1.0 / c.n);
    const double lambda = theta / c.n + c.lambda0;

    // Iterate phi from the isometric-latitude parameter t.
    double phi = kPi / 2.0 - 2.0 * std::atan(t);
    for (int i = 0; i < 12; ++i) {
        const double sin_phi = std::sin(phi);
        const double factor = std::pow((1.0 - kE * sin_phi) / (1.0 + kE * sin_phi), kE / 2.0);
        const double next = kPi / 2.0 - 2.0 * std::atan(t * factor);
        if (std::abs(next - phi) < 1e-12) {
            phi = next;
            break;
        }
        phi = next;
    }

    GeoCoordinate out;
    out.latitude = phi * kRad2Deg;
    out.longitude = lambda * kRad2Deg;
    return out;
}

void state_plane_li_ft_from_wgs84(const GeoCoordinate& coordinate,
                                  double& easting_ft, double& northing_ft) {
    const LccNyLi& c = lcc();
    const double phi = coordinate.latitude * kDeg2Rad;
    const double lambda = coordinate.longitude * kDeg2Rad;

    const double t = LccNyLi::t_of(phi);
    const double rho = kA * c.big_f * std::pow(t, c.n);
    const double theta = c.n * (lambda - c.lambda0);

    const double easting = c.e0_m + rho * std::sin(theta);
    const double northing = c.n0_m + c.rho0 - rho * std::cos(theta);
    easting_ft = easting / kUsFootM;
    northing_ft = northing / kUsFootM;
}

GeoCoordinate wgs84_from_utm18n(const ProjXY& metres) {
    const double e_prime2 = kE2 / (1.0 - kE2);
    const double m = (metres.y - kUtmN0) / kUtmK0;
    const double e1 = (1.0 - std::sqrt(1.0 - kE2)) / (1.0 + std::sqrt(1.0 - kE2));
    const double mu = m / (kA * (1.0 - kE2 / 4.0 - 3.0 * kE2 * kE2 / 64.0 -
                                 5.0 * kE2 * kE2 * kE2 / 256.0));

    const double e1_2 = e1 * e1;
    const double e1_3 = e1_2 * e1;
    const double e1_4 = e1_3 * e1;
    const double phi1 = mu +
        (3.0 * e1 / 2.0 - 27.0 * e1_3 / 32.0) * std::sin(2.0 * mu) +
        (21.0 * e1_2 / 16.0 - 55.0 * e1_4 / 32.0) * std::sin(4.0 * mu) +
        (151.0 * e1_3 / 96.0) * std::sin(6.0 * mu) +
        (1097.0 * e1_4 / 512.0) * std::sin(8.0 * mu);

    const double sin_phi1 = std::sin(phi1);
    const double cos_phi1 = std::cos(phi1);
    const double tan_phi1 = std::tan(phi1);
    const double c1 = e_prime2 * cos_phi1 * cos_phi1;
    const double t1 = tan_phi1 * tan_phi1;
    const double n1 = kA / std::sqrt(1.0 - kE2 * sin_phi1 * sin_phi1);
    const double r1 = kA * (1.0 - kE2) / std::pow(1.0 - kE2 * sin_phi1 * sin_phi1, 1.5);
    const double d = (metres.x - kUtmE0) / (n1 * kUtmK0);

    const double d2 = d * d;
    const double d3 = d2 * d;
    const double d4 = d3 * d;
    const double d5 = d4 * d;
    const double d6 = d5 * d;

    const double phi = phi1 - (n1 * tan_phi1 / r1) *
        (d2 / 2.0 -
         (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * e_prime2) * d4 / 24.0 +
         (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * e_prime2 - 3.0 * c1 * c1) *
             d6 / 720.0);
    const double lambda = kUtmLambda0 +
        (d - (1.0 + 2.0 * t1 + c1) * d3 / 6.0 +
         (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * e_prime2 + 24.0 * t1 * t1) * d5 /
             120.0) /
            cos_phi1;

    GeoCoordinate out;
    out.latitude = phi * kRad2Deg;
    out.longitude = lambda * kRad2Deg;
    return out;
}

ProjXY utm18n_from_wgs84(const GeoCoordinate& coordinate) {
    const double e_prime2 = kE2 / (1.0 - kE2);
    const double phi = coordinate.latitude * kDeg2Rad;
    const double lambda = coordinate.longitude * kDeg2Rad;

    const double sin_phi = std::sin(phi);
    const double cos_phi = std::cos(phi);
    const double tan_phi = std::tan(phi);
    const double n = kA / std::sqrt(1.0 - kE2 * sin_phi * sin_phi);
    const double t = tan_phi * tan_phi;
    const double c = e_prime2 * cos_phi * cos_phi;
    const double a_term = (lambda - kUtmLambda0) * cos_phi;
    const double m = meridian_arc(phi);

    const double a2 = a_term * a_term;
    const double a3 = a2 * a_term;
    const double a4 = a3 * a_term;
    const double a5 = a4 * a_term;
    const double a6 = a5 * a_term;

    ProjXY out;
    out.x = kUtmE0 + kUtmK0 * n *
        (a_term + (1.0 - t + c) * a3 / 6.0 +
         (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * e_prime2) * a5 / 120.0);
    out.y = kUtmN0 + kUtmK0 *
        (m + n * tan_phi *
                 (a2 / 2.0 + (5.0 - t + 9.0 * c + 4.0 * c * c) * a4 / 24.0 +
                  (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * e_prime2) * a6 / 720.0));
    return out;
}

GeoCoordinate wgs84_from_source(HorizontalCrs source, double component0, double component1) {
    switch (source) {
        case HorizontalCrs::StatePlaneNyLongIslandFt:
            return wgs84_from_state_plane_li_ft(component0, component1);
        case HorizontalCrs::Utm18N:
            return wgs84_from_utm18n({component0, component1});
        case HorizontalCrs::Wgs84Lonlat:
        case HorizontalCrs::Unknown:
        default:
            // GeoJSON stores [lon, lat]; component0 = lon, component1 = lat.
            return GeoCoordinate{component1, component0, 0.0};
    }
}

} // namespace agbot::worldgen
