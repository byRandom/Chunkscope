// Game oracle: calls the REAL Minecraft: Java Edition classes and dumps
// reference outputs that the Rust test-suite must reproduce bit-for-bit.
//
// Run it through parity/game-oracle/run.sh, which downloads the official
// server jar from Mojang and puts the game and its libraries on the classpath.
// Only the generated data is committed, never game code.

import java.io.IOException;
import java.io.PrintWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.SplittableRandom;
import java.util.function.LongFunction;

import com.google.gson.JsonParser;
import com.mojang.serialization.JsonOps;
import it.unimi.dsi.fastutil.doubles.DoubleArrayList;
import it.unimi.dsi.fastutil.doubles.DoubleList;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.Enumeration;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

import net.minecraft.util.Mth;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.PositionalRandomFactory;
import net.minecraft.world.level.levelgen.RandomSupport;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.synth.Noise;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.world.level.levelgen.synth.PerlinNoise;

public class GameOracle {
    static Path out;
    // Deterministic generator for the *inputs*; never part of what is tested.
    static final SplittableRandom INPUTS = new SplittableRandom(0xC4A7_5C0BEL);

    public static void main(String[] args) throws IOException {
        out = Path.of(args[0]);
        Files.createDirectories(out);
        try (PrintWriter w = writer("VERSION")) {
            w.println(args[1]);
        }
        support();
        sources("xoroshiro.txt", XoroshiroRandomSource::new);
        sources("legacy.txt", LegacyRandomSource::new);
        worldgen("worldgen_legacy.txt", LegacyRandomSource::new);
        worldgen("worldgen_xoroshiro.txt", XoroshiroRandomSource::new);
        slime();
        // Added after the RNG fixtures so their inputs stay unchanged.
        Map<String, NormalNoise> vanilla = loadVanillaNoises();
        perlin();
        normalNoiseOctaves(vanilla);
        vanillaNoiseSamples(vanilla);
        writeVanillaTable(vanilla, Path.of(args[2]));
        System.out.println("game fixtures written for Minecraft " + args[1]);
    }

    static List<Long> seeds(int extra) {
        List<Long> s = new ArrayList<>(List.of(0L, 1L, -1L, 42L, Long.MIN_VALUE, Long.MAX_VALUE,
                RandomSupport.SILVER_RATIO_64, // upgrade gives lo == 0
                -4172144997902289642L));
        for (int i = 0; i < extra; i++) s.add(INPUTS.nextLong());
        return s;
    }

    static int[] coords() {
        return new int[] {0, 1, -1, 16, -17, 30_000_000, -30_000_000, 46_341, Integer.MAX_VALUE, Integer.MIN_VALUE,
                INPUTS.nextInt(), INPUTS.nextInt(-1_000_000, 1_000_000)};
    }

    static final List<String> NAMES = List.of("", "minecraft:offset", "minecraft:temperature",
            "minecraft:vegetation", "minecraft:continentalness", "minecraft:erosion", "minecraft:ridge",
            "minecraft:aquifer", "minecraft:ore", "octave_-3", "é", "日本", "🎮");

    // ---- RandomSupport + Mth.getSeed --------------------------------------
    static void support() throws IOException {
        try (PrintWriter w = writer("support.txt")) {
            for (long seed : seeds(200)) {
                RandomSupport.Seed128bit s = RandomSupport.upgradeSeedTo128bit(seed);
                w.println("mix " + seed + " " + RandomSupport.mixStafford13(seed) + " " + s.seedLo() + " " + s.seedHi());
            }
            List<String> names = new ArrayList<>(NAMES);
            for (int i = 0; i < 200; i++) names.add("minecraft:n" + INPUTS.nextInt(1_000_000));
            for (String n : names) {
                RandomSupport.Seed128bit s = RandomSupport.seedFromHashOf(n);
                w.println("md5 " + hex(n) + " " + s.seedLo() + " " + s.seedHi());
            }
            int[] c = coords();
            for (int x : c) for (int y : new int[] {-64, 0, 63, 320, Integer.MIN_VALUE}) for (int z : c) {
                w.println("getseed " + x + " " + y + " " + z + " " + Mth.getSeed(x, y, z));
            }
        }
    }

    // ---- RandomSource call script (shared by both algorithms) -------------
    // The Rust test runs exactly the same script; token names make any drift
    // between the two scripts obvious.
    static final int[] BOUNDS = {1, 2, 3, 5, 7, 10, 16, 100, 1000, 1 << 30, (1 << 30) + 1, 1500000001,
            Integer.MAX_VALUE};

    static void script(StringBuilder sb, RandomSource r) {
        for (int i = 0; i < 4; i++) tok(sb, "nextInt", r.nextInt());
        for (int b : BOUNDS) tok(sb, "nextInt(" + b + ")", r.nextInt(b));
        for (int i = 0; i < 3; i++) tok(sb, "nextLong", r.nextLong());
        for (int i = 0; i < 3; i++) tok(sb, "nextFloat", Integer.toHexString(Float.floatToRawIntBits(r.nextFloat())));
        for (int i = 0; i < 3; i++) tok(sb, "nextDouble", Long.toHexString(Double.doubleToRawLongBits(r.nextDouble())));
        for (int i = 0; i < 4; i++) tok(sb, "nextBoolean", r.nextBoolean());
        tok(sb, "between(-5,5)", r.nextIntBetweenInclusive(-5, 5));
        tok(sb, "between(min+2,0)", r.nextIntBetweenInclusive(Integer.MIN_VALUE + 2, 0));
        tok(sb, "range(-100,100)", r.nextInt(-100, 100));
        tok(sb, "triangle", Long.toHexString(Double.doubleToRawLongBits(r.triangle(0.5, 2.0))));
        tok(sb, "triangleF", Integer.toHexString(Float.floatToRawIntBits(r.triangle(0.5f, 2.0f))));
        r.consumeCount(5);
        tok(sb, "afterConsume", r.nextLong());
    }

    static void sources(String file, LongFunction<RandomSource> ctor) throws IOException {
        try (PrintWriter w = writer(file)) {
            for (long seed : seeds(150)) {
                StringBuilder sb = new StringBuilder().append(seed);
                RandomSource r = ctor.apply(seed);
                script(sb, r);

                RandomSource f = r.fork();
                tok(sb, "fork", f.nextLong() + "/" + f.nextLong());
                tok(sb, "parentAfterFork", r.nextLong());

                PositionalRandomFactory p = r.forkPositional();
                tok(sb, "at(0,0,0)", p.at(0, 0, 0).nextLong());
                tok(sb, "at(17,-64,-300)", p.at(17, -64, -300).nextLong());
                tok(sb, "at(big)", p.at(30_000_000, 319, -30_000_000).nextLong());
                for (String n : NAMES) tok(sb, "hash(" + hex(n) + ")", p.fromHashOf(n).nextLong());
                tok(sb, "fromSeed(123)", p.fromSeed(123L).nextLong());
                tok(sb, "fromSeed(" + seed + ")", p.fromSeed(seed).nextLong());

                r.setSeed(seed ^ 0x1234_5678L);
                tok(sb, "setSeed", r.nextLong());
                w.println(sb);
            }
        }
    }

    // ---- WorldgenRandom ----------------------------------------------------
    static void worldgen(String file, LongFunction<RandomSource> ctor) throws IOException {
        try (PrintWriter w = writer(file)) {
            for (long seed : seeds(150)) {
                StringBuilder sb = new StringBuilder().append(seed);
                int x = INPUTS.nextInt(-30_000_000, 30_000_000), z = INPUTS.nextInt(-30_000_000, 30_000_000);
                int step = INPUTS.nextInt(11), index = INPUTS.nextInt(40);
                int rx = INPUTS.nextInt(-60_000, 60_000), rz = INPUTS.nextInt(-60_000, 60_000);
                int salt = INPUTS.nextInt();
                sb.append(' ').append(x).append(' ').append(z).append(' ').append(index).append(' ').append(step)
                        .append(' ').append(rx).append(' ').append(rz).append(' ').append(salt);

                WorldgenRandom r = new WorldgenRandom(ctor.apply(0L));
                long deco = r.setDecorationSeed(seed, x, z);
                tok(sb, "decorationSeed", deco);
                r.setFeatureSeed(deco, index, step);
                script(sb, r);
                tok(sb, "count", r.getCount());

                r.setLargeFeatureSeed(seed, x >> 4, z >> 4);
                tok(sb, "largeFeature", r.nextInt() + "/" + r.nextInt(16));

                r.setLargeFeatureWithSalt(seed, rx, rz, salt);
                tok(sb, "withSalt", r.nextInt(24) + "/" + r.nextInt(24) + "/" + Integer.toHexString(Float.floatToRawIntBits(r.nextFloat())));

                RandomSource f = r.fork();
                tok(sb, "fork", f.nextLong());
                tok(sb, "forkPositional", r.forkPositional().at(x, 0, z).nextLong());
                tok(sb, "finalCount", r.getCount());
                w.println(sb);
            }
        }
    }

    // ---- slime chunks, straight from the game ------------------------------
    static void slime() throws IOException {
        try (PrintWriter w = writer("slime.txt")) {
            for (long seed : seeds(10)) {
                StringBuilder grid = new StringBuilder();
                for (int x = -32; x < 32; x++) for (int z = -32; z < 32; z++) {
                    grid.append(WorldgenRandom.seedSlimeChunk(x, z, seed, 987234911L).nextInt(10) == 0 ? '1' : '0');
                }
                w.println("grid " + seed + " -32 32 " + grid);
            }
        }
    }

    static void tok(StringBuilder sb, String name, Object v) {
        sb.append(' ').append(name).append('=').append(v);
    }

    static String hex(String s) {
        return "x" + java.util.HexFormat.of().formatHex(s.getBytes(java.nio.charset.StandardCharsets.UTF_8));
    }

    static PrintWriter writer(String name) throws IOException {
        return new PrintWriter(Files.newBufferedWriter(out.resolve(name)));
    }

    // ---- noise (levelgen.synth) ---------------------------------------------
    static final double[] SPECIAL_COORDS = {0.0, 0.5, -0.5, 1.0, -1.0, 15.999, 1e-9, -1e-9, 1_000.25, -12_345.678,
            16_777_215.5, 16_777_216.0, -16_777_216.0, 16_777_216.5, 33_554_432.0, 30_000_000.0, -30_000_000.0,
            29_999_999.999, 1e9, -1e9};

    static double[][] samplePoints(int random) {
        List<double[]> pts = new ArrayList<>();
        for (double c : SPECIAL_COORDS) pts.add(new double[] {c, c * 0.25, -c});
        for (int i = 0; i < random; i++) {
            double scale = switch (i % 3) { case 0 -> 64.0; case 1 -> 10_000.0; default -> 3.0e7; };
            pts.add(new double[] {(INPUTS.nextDouble() * 2 - 1) * scale, (INPUTS.nextDouble() * 2 - 1) * 320.0,
                    (INPUTS.nextDouble() * 2 - 1) * scale});
        }
        return pts.toArray(new double[0][]);
    }

    static String bits(float f) {
        return Integer.toHexString(Float.floatToRawIntBits(f));
    }

    static String bits(double d) {
        return Long.toHexString(Double.doubleToRawLongBits(d));
    }

    static void perlin() throws IOException {
        try (PrintWriter w = writer("perlin.txt")) {
            for (String kind : List.of("xoroshiro", "legacy")) {
                for (long seed : seeds(12)) {
                    RandomSource r = kind.equals("xoroshiro") ? new XoroshiroRandomSource(seed) : new LegacyRandomSource(seed);
                    PerlinNoise p = new PerlinNoise(r);
                    StringBuilder sb = new StringBuilder(kind + " " + seed);
                    sb.append(" next=").append(r.nextLong()); // RNG state after construction
                    for (double[] pt : samplePoints(30)) {
                        sb.append(' ').append(pt[0]).append(',').append(pt[1]).append(',').append(pt[2])
                                .append('=').append(bits(p.get(pt[0], pt[1], pt[2])))
                                .append('/').append(bits(p.get(pt[0], pt[2])));
                    }
                    w.println(sb);
                }
            }
        }
    }

    // NormalNoise and its Parameters record are not constructible from outside
    // the package; reflection keeps the oracle on the game's own code paths.
    static Object field(Object o, String name) {
        try {
            Field f = o.getClass().getDeclaredField(name);
            f.setAccessible(true);
            return f.get(o);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }

    static Object call(Object o, String name) {
        try {
            Method m = o.getClass().getDeclaredMethod(name);
            m.setAccessible(true);
            return m.invoke(o);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }

    static NormalNoise normalNoise(double baseAmplitude, int baseOctave, int octaveCount, String normalize, double... modifiers) {
        try {
            Class<?> params = Class.forName("net.minecraft.world.level.levelgen.synth.NormalNoise$Parameters");
            Class<?> norm = Class.forName("net.minecraft.world.level.levelgen.synth.NormalNoise$Normalization");
            Constructor<?> pc = params.getDeclaredConstructor(double.class, int.class, int.class, norm, DoubleList.class);
            pc.setAccessible(true);
            Object p = pc.newInstance(baseAmplitude, baseOctave, octaveCount, norm.getField(normalize).get(null),
                    new DoubleArrayList(modifiers));
            Constructor<NormalNoise> nc = NormalNoise.class.getDeclaredConstructor(params);
            nc.setAccessible(true);
            return nc.newInstance(p);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }

    /** Every worldgen/noise/*.json shipped in the game jar, decoded with the game's own codec. */
    static Map<String, NormalNoise> loadVanillaNoises() throws IOException {
        Map<String, NormalNoise> noises = new LinkedHashMap<>();
        String prefix = "data/minecraft/worldgen/noise/";
        try (ZipFile jar = new ZipFile(Path.of(NormalNoise.class.getProtectionDomain().getCodeSource().getLocation()
                .getPath()).toFile())) {
            List<ZipEntry> entries = new ArrayList<>();
            for (Enumeration<? extends ZipEntry> e = jar.entries(); e.hasMoreElements(); ) {
                ZipEntry z = e.nextElement();
                if (z.getName().startsWith(prefix) && z.getName().endsWith(".json")) entries.add(z);
            }
            entries.sort((a, b) -> a.getName().compareTo(b.getName()));
            for (ZipEntry z : entries) {
                String json = new String(jar.getInputStream(z).readAllBytes(), StandardCharsets.UTF_8);
                NormalNoise n = NormalNoise.DIRECT_CODEC.parse(JsonOps.INSTANCE, JsonParser.parseString(json)).getOrThrow();
                String path = z.getName().substring(prefix.length(), z.getName().length() - ".json".length());
                noises.put("minecraft:" + path, n);
            }
        }
        return noises;
    }

    static void normalNoiseOctaves(Map<String, NormalNoise> vanilla) throws IOException {
        Map<String, NormalNoise> all = new LinkedHashMap<>(vanilla);
        all.put("custom:disabled", normalNoise(1.5, -7, 5, "DISABLED"));
        all.put("custom:legacy", normalNoise(1.0, -8, 6, "LEGACY", 1.0, 0.0, 2.0, 0.0, 1.0, 0.0));
        all.put("custom:legacy_single", normalNoise(0.7, 0, 1, "LEGACY"));
        all.put("custom:zero_mods", normalNoise(2.0, -4, 4, "ENABLED", 0.0, 0.0, 0.0, 0.0));
        all.put("custom:negative_mods", normalNoise(1.0, -6, 3, "ENABLED", -1.0, 0.5, 3.0));
        try (PrintWriter w = writer("normal_octaves.txt")) {
            for (var e : all.entrySet()) {
                NormalNoise n = e.getValue();
                Object p = field(n, "parameters");
                StringBuilder sb = new StringBuilder(e.getKey());
                sb.append(' ').append(bits((double) call(p, "baseAmplitude"))).append(' ').append(call(p, "baseOctave"))
                        .append(' ').append(call(p, "octaveCount")).append(' ').append(call(p, "normalize"))
                        .append(" [");
                for (double m : (DoubleList) call(p, "amplitudeModifiers")) sb.append(bits(m)).append(',');
                sb.append("] factor=").append(bits((double) field(n, "normalizationFactor")));
                for (Object o : (List<?>) field(n, "octaves")) {
                    sb.append(' ').append(call(o, "octaveIndex")).append(':').append(bits((double) call(o, "frequency")))
                            .append(':').append(bits((double) call(o, "amplitude")));
                }
                // sample the custom ones too, seeded like world noises
                Noise s = n.create(new XoroshiroRandomSource(42L).forkPositional().fromHashOf(e.getKey()));
                sb.append(" sample=").append(bits(s.get(12.5, -40.0, 7_000.75))).append('/').append(bits(s.get(-3.25, 99.0)));
                w.println(sb);
            }
        }
    }

    static void vanillaNoiseSamples(Map<String, NormalNoise> vanilla) throws IOException {
        double[][] pts = samplePoints(16);
        try (PrintWriter w = writer("noise.txt")) {
            StringBuilder header = new StringBuilder("points");
            for (double[] pt : pts) header.append(' ').append(pt[0]).append(',').append(pt[1]).append(',').append(pt[2]);
            w.println(header);
            for (long seed : List.of(0L, 1L, -1L, Long.MIN_VALUE, 123456789L, INPUTS.nextLong())) {
                // exactly what RandomState + Noises.instantiate do for XOROSHIRO worlds
                PositionalRandomFactory factory = new XoroshiroRandomSource(seed).forkPositional();
                for (var e : vanilla.entrySet()) {
                    Noise n = e.getValue().create(factory.fromHashOf(e.getKey()));
                    StringBuilder sb = new StringBuilder(seed + " " + e.getKey());
                    for (double[] pt : pts) sb.append(' ').append(bits(n.get(pt[0], pt[1], pt[2]))).append('/')
                            .append(bits(n.get(pt[0], pt[2])));
                    w.println(sb);
                }
            }
        }
    }

    /** Emits the vanilla noise parameters as Rust source. */
    static void writeVanillaTable(Map<String, NormalNoise> vanilla, Path rs) throws IOException {
        try (PrintWriter w = new PrintWriter(Files.newBufferedWriter(rs))) {
            w.println("// @generated by parity/game-oracle/GameOracle.java from the Minecraft "
                    + Files.readString(out.resolve("VERSION")).trim() + " data files. Do not edit.");
            w.println("//! Parameters of every vanilla `worldgen/noise` entry.");
            w.println();
            w.println("use std::borrow::Cow;");
            w.println();
            w.println("use super::{NoiseParameters, Normalization};");
            w.println();
            List<String[]> consts = new ArrayList<>();
            for (var e : vanilla.entrySet()) {
                Object p = field(e.getValue(), "parameters");
                String constName = e.getKey().substring("minecraft:".length()).replace('/', '_').toUpperCase();
                consts.add(new String[] {e.getKey(), constName});
                StringBuilder mods = new StringBuilder();
                for (double m : (DoubleList) call(p, "amplitudeModifiers")) {
                    if (mods.length() > 0) mods.append(", ");
                    mods.append(rustF64(m));
                }
                String normalize = call(p, "normalize").toString();
                w.println("/// `" + e.getKey() + "`");
                w.println("pub const " + constName + ": NoiseParameters = NoiseParameters {");
                w.println("    base_amplitude: " + rustF64((double) call(p, "baseAmplitude")) + ",");
                w.println("    base_octave: " + call(p, "baseOctave") + ",");
                w.println("    octave_count: " + call(p, "octaveCount") + ",");
                w.println("    normalize: Normalization::" + normalize.charAt(0) + normalize.substring(1).toLowerCase() + ",");
                w.println("    amplitude_modifiers: Cow::Borrowed(&[" + mods + "]),");
                w.println("};");
                w.println();
            }
            w.println("/// All vanilla noises by registry name.");
            w.println("pub const ALL: [(&str, NoiseParameters); " + consts.size() + "] = [");
            for (String[] c : consts) w.println("    (\"" + c[0] + "\", " + c[1] + "),");
            w.println("];");
        }
    }

    /** Shortest round-trip decimal (JDK 19+ Double.toString), always with a decimal point. */
    static String rustF64(double d) {
        String s = Double.toString(d);
        return s.replace("E", "e");
    }
}
