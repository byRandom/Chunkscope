// Parity oracle: runs the REAL JDK and dumps reference outputs that the Rust
// test-suite must reproduce bit-for-bit.
//
// Usage (from repo root):  java parity/oracle/Oracle.java
// Use the same major JDK version Minecraft ships with (Java 21 for 1.20.5+).

import java.io.IOException;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Random;
import java.util.SplittableRandom;

public class Oracle {
    static final Path FIXTURES = Path.of("parity/fixtures");
    // Deterministic generator for the *inputs*; never part of what is tested.
    static final SplittableRandom INPUTS = new SplittableRandom(0x5EED_3A9L);

    public static void main(String[] args) throws IOException {
        Files.createDirectories(FIXTURES);
        random();
        strictLog();
        seeds();
        slime();
        digitTable();
        streamSum(); // after the others so their inputs stay unchanged
        mathRound();
        System.out.println("fixtures written by Java " + System.getProperty("java.version"));
    }

    static List<Long> testSeeds(int extra) {
        List<Long> seeds = new ArrayList<>(List.of(0L, 1L, -1L, 42L, Long.MIN_VALUE, Long.MAX_VALUE,
                0x5DEECE66DL, 1L << 48, (1L << 48) - 1, -4172144997902289642L));
        for (int i = 0; i < extra; i++) seeds.add(INPUTS.nextLong());
        return seeds;
    }

    // ---- java.util.Random ------------------------------------------------
    // The Rust test runs exactly the same call script; token names make any
    // drift between the two scripts obvious.
    static final int[] BOUNDS = {1, 2, 3, 5, 7, 10, 16, 100, 1000, 1 << 30, (1 << 30) + 1,
            1500000001, Integer.MAX_VALUE};

    static void random() throws IOException {
        try (PrintWriter out = writer("random.txt")) {
            for (long seed : testSeeds(300)) {
                Random r = new Random(seed);
                StringBuilder sb = new StringBuilder().append(seed);
                for (int i = 0; i < 4; i++) tok(sb, "nextInt", r.nextInt());
                for (int b : BOUNDS) tok(sb, "nextInt(" + b + ")", r.nextInt(b));
                for (int i = 0; i < 3; i++) tok(sb, "nextLong", r.nextLong());
                for (int i = 0; i < 3; i++) tok(sb, "nextFloat", Integer.toHexString(Float.floatToRawIntBits(r.nextFloat())));
                for (int i = 0; i < 3; i++) tok(sb, "nextDouble", Long.toHexString(Double.doubleToRawLongBits(r.nextDouble())));
                for (int i = 0; i < 4; i++) tok(sb, "nextBoolean", r.nextBoolean());
                for (int i = 0; i < 7; i++) tok(sb, "nextGaussian", Long.toHexString(Double.doubleToRawLongBits(r.nextGaussian())));
                r.setSeed(seed + 1); // must discard the cached gaussian
                tok(sb, "setSeed+nextGaussian", Long.toHexString(Double.doubleToRawLongBits(r.nextGaussian())));
                out.println(sb);
            }
        }
    }

    static void tok(StringBuilder sb, String name, Object v) {
        sb.append(' ').append(name).append('=').append(v);
    }

    // ---- StrictMath.log --------------------------------------------------
    static void strictLog() throws IOException {
        List<Double> xs = new ArrayList<>(List.of(0.0, -0.0, 1.0, -1.0, 2.0, 0.5, Math.E, Double.MIN_VALUE,
                Double.MIN_NORMAL, Double.MAX_VALUE, Double.POSITIVE_INFINITY, Double.NaN,
                Math.nextUp(1.0), Math.nextDown(1.0), 1.0 + 1e-7, 1.0 - 1e-7, 4.9e-300));
        for (int i = 0; i < 20000; i++) xs.add(INPUTS.nextDouble());          // gaussian's domain
        for (int i = 0; i < 5000; i++) xs.add(Double.longBitsToDouble(INPUTS.nextLong() & Long.MAX_VALUE));
        for (int i = 0; i < 2000; i++) xs.add(1.0 + (INPUTS.nextDouble() - 0.5) * 1e-5); // |f| tiny paths
        try (PrintWriter out = writer("strict_log.txt")) {
            for (double x : xs) {
                out.println(Long.toHexString(Double.doubleToRawLongBits(x)) + " "
                        + Long.toHexString(Double.doubleToRawLongBits(StrictMath.log(x))));
            }
        }
    }

    // ---- seed text -> world seed (WorldOptions.parseSeed) ---------------
    static String parseSeed(String input) {
        String s = input.trim();
        if (s.isEmpty()) return "random";
        try {
            return Long.toString(Long.parseLong(s));
        } catch (NumberFormatException e) {
            return Long.toString(s.hashCode());
        }
    }

    static void seeds() throws IOException {
        List<String> inputs = new ArrayList<>(List.of("", "   ", "\t\n", "0", "-0", "+0", "123", "-123", "+123",
                "9223372036854775807", "9223372036854775808", "-9223372036854775808", "-9223372036854775809",
                "00000000000000000000000000000000000042", "Glacier", "gravel", "minecraft", "Minecraft",
                " 42 ", "\t42\n", " 42", "42 ", "　hello　", "١٢٣", // Arabic-Indic
                "１２３", "-१२", "๑๒", "1٢", "𝟏", // math bold 1 (non-BMP)
                "🎮", "日本語", "-", "+", "--1", "+-1", "1_000", "1e5", "0x10", "1 2",
                "a", "aa", "polishedcobblestone", "é", "é", "\u0000", "\u001f42\u001f"));
        String alphabet = "0123456789-+ \tabcXYZ_ ٠٩０é世";
        for (int i = 0; i < 3000; i++) {
            int len = INPUTS.nextInt(24);
            StringBuilder sb = new StringBuilder();
            boolean digitsOnly = INPUTS.nextInt(3) == 0;
            for (int j = 0; j < len; j++) {
                sb.append(digitsOnly ? (char) ('0' + INPUTS.nextInt(10))
                        : alphabet.charAt(INPUTS.nextInt(alphabet.length())));
            }
            inputs.add(sb.toString());
        }
        HexFormat hex = HexFormat.of();
        try (PrintWriter out = writer("seeds.txt")) {
            for (String s : inputs) {
                // hex of UTF-8 so whitespace/control chars survive the file format
                out.println("x" + hex.formatHex(s.getBytes(StandardCharsets.UTF_8)) + " " + parseSeed(s));
            }
        }
    }

    // ---- slime chunks (WorldgenRandom.seedSlimeChunk) -------------------
    static boolean isSlimeChunk(long seed, int x, int z) {
        return new Random(seed + (long) (x * x * 4987142) + (long) (x * 5947611)
                + (long) (z * z) * 4392871L + (long) (z * 389711) ^ 987234911L).nextInt(10) == 0;
    }

    static void slime() throws IOException {
        int[] special = {46340, 46341, -46341, 1875000, -1875000, 1875004, -1875005, 30000000 / 16,
                Integer.MAX_VALUE, Integer.MIN_VALUE, 65536, -65537};
        try (PrintWriter out = writer("slime.txt")) {
            for (long seed : testSeeds(15)) {
                // grid near origin, one bit per chunk, row-major x then z
                StringBuilder grid = new StringBuilder();
                for (int x = -32; x < 32; x++) for (int z = -32; z < 32; z++) grid.append(isSlimeChunk(seed, x, z) ? '1' : '0');
                out.println("grid " + seed + " -32 32 " + grid);
                for (int x : special) for (int z : special) {
                    out.println("at " + seed + " " + x + " " + z + " " + (isSlimeChunk(seed, x, z) ? 1 : 0));
                }
                for (int i = 0; i < 200; i++) {
                    int x = INPUTS.nextInt(-1875000, 1875001), z = INPUTS.nextInt(-1875000, 1875001);
                    out.println("at " + seed + " " + x + " " + z + " " + (isSlimeChunk(seed, x, z) ? 1 : 0));
                }
            }
        }
    }

    // ---- DoubleStream.sum (Kahan compensated) -----------------------------
    static void streamSum() throws IOException {
        List<double[]> cases = new ArrayList<>(List.of(new double[0], new double[] {1e100, 1.0, -1e100},
                new double[] {Double.POSITIVE_INFINITY, 1.0}, new double[] {Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY},
                new double[] {Double.NaN, 1.0}, new double[] {0.1, 0.2, 0.3}, new double[] {-0.0}, new double[] {-0.0, -0.0}));
        for (int i = 0; i < 2000; i++) {
            double[] xs = new double[INPUTS.nextInt(1, 40)];
            for (int j = 0; j < xs.length; j++) {
                xs[j] = switch (INPUTS.nextInt(4)) {
                    case 0 -> INPUTS.nextDouble();
                    case 1 -> (INPUTS.nextDouble() - 0.5) * Math.pow(2, INPUTS.nextInt(-60, 60));
                    case 2 -> Math.pow(0.5, INPUTS.nextInt(12)) * INPUTS.nextInt(1, 4);
                    default -> Double.longBitsToDouble(INPUTS.nextLong());
                };
            }
            cases.add(xs);
        }
        try (PrintWriter out = writer("stream_sum.txt")) {
            for (double[] xs : cases) {
                StringBuilder sb = new StringBuilder(Long.toHexString(Double.doubleToRawLongBits(
                        java.util.Arrays.stream(xs).sum())));
                for (double x : xs) sb.append(' ').append(Long.toHexString(Double.doubleToRawLongBits(x)));
                out.println(sb);
            }
        }
    }

    // ---- Math.round(double) ------------------------------------------------
    static void mathRound() throws IOException {
        List<Double> xs = new ArrayList<>(List.of(0.0, -0.0, 0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 0.49999999999999994,
                -0.49999999999999994, 4503599627370495.5, -4503599627370495.5, 4503599627370496.0, 9.2233720368547758E18,
                -9.2233720368547758E18, 1e300, -1e300, Double.MIN_VALUE, Double.NaN, Double.POSITIVE_INFINITY,
                Double.NEGATIVE_INFINITY, Math.nextDown(0.5), Math.nextUp(-0.5)));
        for (int i = 0; i < 3000; i++) {
            xs.add(switch (i % 3) {
                case 0 -> (INPUTS.nextDouble() - 0.5) * 4000.0;
                case 1 -> INPUTS.nextInt(-100000, 100000) + 0.5;
                default -> Double.longBitsToDouble(INPUTS.nextLong());
            });
        }
        try (PrintWriter out = writer("math_round.txt")) {
            for (double x : xs) out.println(Long.toHexString(Double.doubleToRawLongBits(x)) + " " + Math.round(x));
        }
    }

    // ---- Character.digit(ch, 10) table -> Rust source --------------------
    static void digitTable() throws IOException {
        Path rs = Path.of("crates/java-compat/src/digit_table.rs");
        try (PrintWriter out = new PrintWriter(Files.newBufferedWriter(rs))) {
            out.println("// @generated by parity/oracle/Oracle.java from Java " + Runtime.version().feature() + ". Do not edit.");
            out.println("// Runs of UTF-16 units accepted by `Character.digit(ch, 10)`: (first unit, its value).");
            out.println("pub(crate) const DIGIT_RANGES: &[(u16, u8)] = &[");
            int prev = -2, prevVal = -2;
            for (int c = 0; c <= 0xFFFF; c++) {
                int d = Character.digit((char) c, 10);
                if (d >= 0 && !(c == prev + 1 && d == prevVal + 1)) {
                    out.printf("    (0x%04X, %d),%n", c, d);
                }
                if (d < 0) { prev = -2; prevVal = -2; } else { prev = c; prevVal = d; }
                // a run ends where digits stop; mark the gap so lookups past it fail
                if (d >= 0 && Character.digit((char) (c + 1), 10) < 0 && d < 9) {
                    throw new IllegalStateException("digit run not ending at 9 at U+" + Integer.toHexString(c));
                }
            }
            out.println("];");
        }
    }

    static PrintWriter writer(String name) throws IOException {
        return new PrintWriter(Files.newBufferedWriter(FIXTURES.resolve(name)));
    }
}
