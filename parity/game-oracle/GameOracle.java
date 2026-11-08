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

import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.CubicSpline;
import net.minecraft.util.Mth;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.densityfunction.DensityFunction;
import net.minecraft.world.level.levelgen.densityfunction.DensitySampler;
import net.minecraft.world.level.levelgen.densityfunction.SamplerContext;
import net.minecraft.world.level.levelgen.densityfunction.op.SplineFunction;
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

    public static void main(String[] args) throws Exception {
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
        // Biomes need the game's registries (built from code, no world needed).
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        HolderLookup.Provider lookup = VanillaRegistries.createWorldLookup();
        writeBiomeTables(lookup, Path.of(args[3]));
        climateAndBiomes(lookup);
        writeStructureSets(lookup, Path.of(args[4]));
        structurePlacement(lookup);
        frequencyReduction();
        strongholds(lookup);
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
            for (Class<?> k = o.getClass(); k != null; k = k.getSuperclass()) {
                try {
                    Method m = k.getDeclaredMethod(name);
                    m.setAccessible(true);
                    return m.invoke(o);
                } catch (NoSuchMethodException e) {
                    // look in the superclass
                }
            }
            throw new NoSuchMethodException(o.getClass().getName() + "." + name + "()");
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

    // ---- biomes ---------------------------------------------------------------
    /** Breadth-first search for the first object of a class inside a record/object graph. */
    static <T> T find(Object root, Class<T> type) {
        java.util.ArrayDeque<Object> queue = new java.util.ArrayDeque<>(List.of(root));
        java.util.Set<Object> seen = java.util.Collections.newSetFromMap(new java.util.IdentityHashMap<>());
        while (!queue.isEmpty()) {
            Object o = queue.poll();
            if (o == null || !seen.add(o)) continue;
            if (type.isInstance(o)) return type.cast(o);
            if (o instanceof Holder<?> h) { queue.add(h.value()); continue; }
            if (o instanceof Iterable<?> it) { it.forEach(queue::add); continue; }
            for (Class<?> k = o.getClass(); k != null && k != Object.class && k != Record.class; k = k.getSuperclass()) {
                if (!k.getName().startsWith("net.minecraft")) break;
                for (Field f : k.getDeclaredFields()) {
                    if (java.lang.reflect.Modifier.isStatic(f.getModifiers()) || f.getType().isPrimitive()) continue;
                    try { f.setAccessible(true); queue.add(f.get(o)); } catch (ReflectiveOperationException | RuntimeException e) { /* skip */ }
                }
            }
        }
        throw new IllegalStateException("no " + type.getSimpleName() + " in " + root);
    }

    static DensityFunction densityFunction(HolderLookup.Provider lookup, String name) {
        return lookup.lookupOrThrow(Registries.DENSITY_FUNCTION)
                .getOrThrow(ResourceKey.create(Registries.DENSITY_FUNCTION, Identifier.parse(name))).value();
    }

    /** The registry name a spline coordinate refers to. */
    static String coordinateName(SplineFunction.Coordinate c) {
        Holder<?> h = find(c.function(), Holder.class);
        return h.unwrapKey().orElseThrow().identifier().toString();
    }

    static String rustF32(float f) {
        String s = Float.toString(f).replace("E", "e");
        return s;
    }

    static void rustSpline(StringBuilder out, CubicSpline<SplineFunction.Coordinate> spline, String indent) {
        if (spline instanceof CubicSpline.Constant<SplineFunction.Coordinate> c) {
            out.append("Spline::Constant(").append(rustF32(c.value())).append(")");
            return;
        }
        var m = (CubicSpline.Multipoint<SplineFunction.Coordinate>) spline;
        String coordinate = switch (coordinateName(m.coordinate())) {
            case "minecraft:overworld/continents" -> "Continents";
            case "minecraft:overworld/erosion" -> "Erosion";
            case "minecraft:overworld/ridges_folded" -> "RidgesFolded";
            default -> throw new IllegalStateException("unknown spline coordinate " + coordinateName(m.coordinate()));
        };
        out.append("Spline::Multipoint(&Multipoint {\n");
        out.append(indent).append("    coordinate: SplineCoordinate::").append(coordinate).append(",\n");
        out.append(indent).append("    locations: &[");
        for (int i = 0; i < m.locations().length; i++) out.append(i == 0 ? "" : ", ").append(rustF32(m.locations()[i]));
        out.append("],\n").append(indent).append("    derivatives: &[");
        for (int i = 0; i < m.derivatives().length; i++) out.append(i == 0 ? "" : ", ").append(rustF32(m.derivatives()[i]));
        out.append("],\n").append(indent).append("    values: &[\n");
        for (CubicSpline<SplineFunction.Coordinate> v : m.values()) {
            out.append(indent).append("        ");
            rustSpline(out, v, indent + "        ");
            out.append(",\n");
        }
        out.append(indent).append("    ],\n").append(indent).append("})");
    }

    static void writeBiomeTables(HolderLookup.Provider lookup, Path rs) throws IOException {
        StringBuilder out = new StringBuilder();
        out.append("// @generated by parity/game-oracle/GameOracle.java from Minecraft ")
                .append(Files.readString(this_out().resolve("VERSION")).trim()).append(". Do not edit.\n");
        out.append("//! Overworld terrain spline and biome parameter list, exported from the game.\n\n");
        out.append("use super::climate::Parameter;\nuse super::spline::{Multipoint, Spline, SplineCoordinate};\n\n");

        // overworld/offset = cache(lerp(blend_alpha, blend_offset, add(<constant>, spline)))
        DensityFunction offset = densityFunction(lookup, "minecraft:overworld/offset");
        SplineFunction spline = find(offset, SplineFunction.class);
        var add = find(offset, net.minecraft.world.level.levelgen.densityfunction.op.BinaryFunction.class);
        Object constant = add.left() instanceof SplineFunction ? add.right() : add.left();
        float offsetAdd = (float) call(constant, "value");
        out.append("/// The constant added to the spline in `minecraft:overworld/offset`.\n");
        out.append("pub const OFFSET_ADD: f32 = ").append(rustF32(offsetAdd)).append(";\n\n");
        out.append("/// The spline of `minecraft:overworld/offset`.\npub const OFFSET: Spline = ");
        rustSpline(out, spline.spline(), "");
        out.append(";\n\n");

        var params = MultiNoiseBiomeSourceParameterList.knownPresets().get(MultiNoiseBiomeSourceParameterList.Preset.OVERWORLD);
        List<String> biomes = new ArrayList<>();
        List<Climate.Parameter> parameters = new ArrayList<>();
        StringBuilder entries = new StringBuilder();
        for (var pair : params.values()) {
            Climate.ParameterPoint p = pair.getFirst();
            String biome = pair.getSecond().identifier().toString();
            if (!biomes.contains(biome)) biomes.add(biome);
            entries.append("    ([");
            Climate.Parameter[] ps = {p.temperature(), p.humidity(), p.continentalness(), p.erosion(), p.depth(), p.weirdness()};
            for (int i = 0; i < 6; i++) {
                if (!parameters.contains(ps[i])) parameters.add(ps[i]);
                entries.append(i == 0 ? "" : ", ").append(parameters.indexOf(ps[i]));
            }
            entries.append("], ").append(p.offset()).append(", ").append(biomes.indexOf(biome)).append("),\n");
        }
        out.append("/// Biome registry names; `Biome(i)` is `BIOMES[i]`.\npub const BIOMES: [&str; ").append(biomes.size()).append("] = [\n");
        for (String b : biomes) out.append("    \"").append(b).append("\",\n");
        out.append("];\n\n/// Distinct climate ranges used by `OVERWORLD`.\npub const PARAMETERS: [Parameter; ")
                .append(parameters.size()).append("] = [\n");
        for (Climate.Parameter p : parameters) out.append("    Parameter::new(").append(p.min()).append(", ").append(p.max()).append("),\n");
        out.append("];\n\n/// The `minecraft:overworld` parameter list, in game order: indices into\n")
                .append("/// `PARAMETERS` for temperature, humidity, continentalness, erosion, depth\n")
                .append("/// and weirdness, then the offset, then the index into `BIOMES`.\n")
                .append("pub static OVERWORLD: [([u16; 6], i64, u16); ").append(params.values().size()).append("] = [\n")
                .append(entries).append("];\n");
        Files.writeString(rs, out);
    }

    static Path this_out() {
        return out;
    }

    static void climateAndBiomes(HolderLookup.Provider lookup) throws IOException {
        NoiseGeneratorSettings settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.OVERWORLD).value();
        var preset = MultiNoiseBiomeSourceParameterList.knownPresets().get(MultiNoiseBiomeSourceParameterList.Preset.OVERWORLD);
        List<Long> worldSeeds = List.of(0L, 1L, -1L, 42L, 123456789L, -4172144997902289642L, INPUTS.nextLong(), INPUTS.nextLong());
        try (PrintWriter climate = writer("climate.txt"); PrintWriter biomes = writer("biomes.txt")) {
            for (long seed : worldSeeds) {
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                Climate.Sampler sampler = rs.createClimateSampler(SamplerContext.EMPTY_UNCACHED);
                DensitySampler.Bound[] fns = {sampler.temperature(), sampler.humidity(), sampler.continentalness(),
                        sampler.erosion(), sampler.depth(), sampler.weirdness()};

                // raw climate values at block positions
                for (int i = 0; i < 300; i++) {
                    int range = switch (i % 3) { case 0 -> 2_000; case 1 -> 200_000; default -> 30_000_000; };
                    int x = INPUTS.nextInt(-range, range + 1), y = INPUTS.nextInt(-80, 340), z = INPUTS.nextInt(-range, range + 1);
                    StringBuilder sb = new StringBuilder(seed + " " + x + " " + y + " " + z);
                    for (DensitySampler.Bound f : fns) sb.append(' ').append(bits(f.sampleValue(x, y, z)));
                    climate.println(sb);
                }

                // biome queries in a fixed order, on a fresh R-tree (its search starts from the last result)
                Climate.ParameterList<ResourceKey<Biome>> list = new Climate.ParameterList<>(preset.values());
                StringBuilder sb = new StringBuilder(String.valueOf(seed));
                for (int qz = -24; qz < 24; qz++) for (int qx = -24; qx < 24; qx++) {
                    query(sb, sampler, list, qx * 8, 16, qz * 8);
                }
                for (int i = 0; i < 400; i++) {
                    int range = i % 2 == 0 ? 50_000 : 7_500_000; // quart coordinates
                    query(sb, sampler, list, INPUTS.nextInt(-range, range), INPUTS.nextInt(-16, 80), INPUTS.nextInt(-range, range));
                }
                biomes.println(sb);
            }
        }
    }

    static void query(StringBuilder sb, Climate.Sampler sampler, Climate.ParameterList<ResourceKey<Biome>> list, int qx, int qy, int qz) {
        Climate.TargetPoint t = sampler.sample(qx, qy, qz);
        String biome = list.findValue(t).identifier().getPath();
        sb.append(' ').append(qx).append(',').append(qy).append(',').append(qz).append('=').append(biome);
    }

    // ---- structure placement --------------------------------------------------
    static List<net.minecraft.core.Holder.Reference<net.minecraft.world.level.levelgen.structure.StructureSet>> structureSets(HolderLookup.Provider lookup) {
        return lookup.lookupOrThrow(Registries.STRUCTURE_SET).listElements()
                .sorted((a, b) -> a.key().identifier().toString().compareTo(b.key().identifier().toString()))
                .toList();
    }

    static void writeStructureSets(HolderLookup.Provider lookup, Path rs) throws IOException {
        StringBuilder out = new StringBuilder();
        out.append("// @generated by parity/game-oracle/GameOracle.java from Minecraft ")
                .append(Files.readString(out().resolve("VERSION")).trim()).append(". Do not edit.\n");
        out.append("//! Every vanilla structure set, exported from the game.\n\n");
        out.append("use super::placement::*;\n\n");
        var sets = structureSets(lookup);
        out.append("/// All vanilla structure sets, sorted by name.\npub static STRUCTURE_SETS: [StructureSet; ").append(sets.size()).append("] = [\n");
        for (var ref : sets) {
            var set = ref.value();
            out.append("    StructureSet {\n        name: \"").append(ref.key().identifier()).append("\",\n        structures: &[");
            boolean first = true;
            for (var e : set.structures()) {
                out.append(first ? "" : ", ").append("(\"").append(e.structure().unwrapKey().orElseThrow().identifier()).append("\", ").append(e.weight()).append(")");
                first = false;
            }
            out.append("],\n        placement: ");
            Object p = set.placement();
            if (p instanceof net.minecraft.world.level.levelgen.structure.placement.RandomSpreadStructurePlacement r) {
                var offset = r.locateOffset();
                Object method = call(r, "frequencyReductionMethod");
                String reduction = switch (((net.minecraft.util.StringRepresentable) method).getSerializedName()) {
                    case "default" -> "Default"; case "legacy_type_1" -> "LegacyType1";
                    case "legacy_type_2" -> "LegacyType2"; case "legacy_type_3" -> "LegacyType3";
                    default -> throw new IllegalStateException(method.toString());
                };
                @SuppressWarnings("unchecked")
                var zone = (java.util.Optional<net.minecraft.world.level.levelgen.structure.placement.AbstractSpreadingStructurePlacement.ExclusionZone>) call(r, "exclusionZone");
                out.append("StructurePlacement::RandomSpread(RandomSpreadPlacement {\n")
                        .append("            locate_offset: (").append(offset.getX()).append(", ").append(offset.getY()).append(", ").append(offset.getZ()).append("),\n")
                        .append("            frequency_reduction: FrequencyReduction::").append(reduction).append(",\n")
                        .append("            frequency: ").append(rustF32((float) call(r, "frequency"))).append(",\n")
                        .append("            salt: ").append(call(r, "salt")).append(",\n")
                        .append("            exclusion_zone: ").append(zone.map(z -> "Some(ExclusionZone { other_set: \""
                                + z.otherSet().unwrapKey().orElseThrow().identifier() + "\", chunk_count: " + z.chunkCount() + " })").orElse("None")).append(",\n")
                        .append("            spacing: ").append(r.spacing()).append(",\n")
                        .append("            separation: ").append(r.separation()).append(",\n")
                        .append("            spread_type: SpreadType::").append(r.spreadType().name().charAt(0)).append(r.spreadType().name().substring(1).toLowerCase()).append(",\n")
                        .append("        }),\n");
            } else if (p instanceof net.minecraft.world.level.levelgen.structure.placement.ConcentricRingsStructurePlacement c) {
                String tag = c.preferredBiomes().unwrapKey().orElseThrow().location().toString();
                StringBuilder biomes = new StringBuilder();
                for (String b : biomeTag(tag)) biomes.append(biomes.length() == 0 ? "" : ", ").append('"').append(b).append('"');
                out.append("StructurePlacement::ConcentricRings {\n            distance: ").append(c.distance())
                        .append(",\n            spread: ").append(c.spread())
                        .append(",\n            count: ").append(c.count()).append(",\n            salt: ").append(call(c, "salt"))
                        .append(",\n            preferred_biomes: \"#").append(tag).append("\",\n            preferred_biome_list: &[")
                        .append(biomes).append("],\n        },\n");
            } else {
                throw new IllegalStateException("unsupported placement " + p);
            }
            out.append("    },\n");
        }
        out.append("];\n");
        Files.writeString(rs, out);
    }

    static Path out() {
        return out;
    }

    static void structurePlacement(HolderLookup.Provider lookup) throws IOException {
        NoiseGeneratorSettings settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.OVERWORLD).value();
        var biomeSource = net.minecraft.world.level.biome.MultiNoiseBiomeSource.createFromPreset(
                lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
                        .getOrThrow(net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists.OVERWORLD));
        var sets = structureSets(lookup);
        List<Long> worldSeeds = List.of(0L, 1L, -1L, 42L, Long.MIN_VALUE, -4172144997902289642L, INPUTS.nextLong(), INPUTS.nextLong());
        try (PrintWriter w = writer("structure_placement.txt")) {
            for (long seed : worldSeeds) {
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                // createForNormal() filters sets by biome tags, which are not bound in a
                // world-less registry lookup; placement checks never use that filter.
                net.minecraft.world.level.chunk.ChunkGeneratorStructureState state;
                try {
                    var ctor = net.minecraft.world.level.chunk.ChunkGeneratorStructureState.class.getDeclaredConstructor(
                            RandomState.class, net.minecraft.world.level.biome.BiomeSource.class, long.class,
                            net.minecraft.world.level.ChunkPos.class, long.class, List.class);
                    ctor.setAccessible(true);
                    state = ctor.newInstance(rs, biomeSource, seed, new net.minecraft.world.level.ChunkPos(0, 0), seed, List.copyOf(sets));
                } catch (ReflectiveOperationException e) {
                    throw new RuntimeException(e);
                }
                for (var ref : sets) {
                    if (!(ref.value().placement() instanceof net.minecraft.world.level.levelgen.structure.placement.RandomSpreadStructurePlacement r)) continue;
                    String name = ref.key().identifier().toString();
                    StringBuilder sb = new StringBuilder(seed + " " + name);
                    if (r.spacing() > 1) {
                        // candidate chunk of each region, and whether the game attempts a structure there
                        for (int rz = -5; rz <= 5; rz++) for (int rx = -5; rx <= 5; rx++) region(sb, state, r, seed, rx, rz);
                        for (int i = 0; i < 40; i++) {
                            int range = 30_000_000 / 16 / r.spacing();
                            region(sb, state, r, seed, INPUTS.nextInt(-range, range), INPUTS.nextInt(-range, range));
                        }
                    } else {
                        // spacing 1: every chunk is a candidate, only the frequency roll matters
                        sb.append(" grid -24 24 ");
                        for (int cz = -24; cz < 24; cz++) for (int cx = -24; cx < 24; cx++) sb.append(r.isStructureChunk(state, cx, cz) ? '1' : '0');
                    }
                    w.println(sb);
                }
            }
        }
    }

    static void region(StringBuilder sb, net.minecraft.world.level.chunk.ChunkGeneratorStructureState state,
            net.minecraft.world.level.levelgen.structure.placement.RandomSpreadStructurePlacement r, long seed, int rx, int rz) {
        var c = r.getPotentialStructureChunk(seed, rx * r.spacing(), rz * r.spacing());
        boolean attempt = r.isStructureChunk(state, c.x(), c.z());
        var locate = r.getLocatePos(c);
        sb.append(' ').append(rx).append(',').append(rz).append('=').append(c.x()).append(',').append(c.z())
                .append(',').append(attempt ? 1 : 0).append(',').append(locate.getX()).append(',').append(locate.getZ());
    }

    /** Every FrequencyReductionMethod directly, including `default`, which no vanilla set uses below frequency 1. */
    static void frequencyReduction() throws IOException {
        var methods = net.minecraft.world.level.levelgen.structure.placement.AbstractSpreadingStructurePlacement.FrequencyReductionMethod.values();
        float[] frequencies = {0.004f, 0.01f, 0.2f, 0.25f, 0.5f, 0.75f, 0.999f};
        try (PrintWriter w = writer("frequency_reduction.txt")) {
            for (int i = 0; i < 400; i++) {
                long seed = i < 4 ? new long[] {0L, 1L, -1L, Long.MIN_VALUE}[i] : INPUTS.nextLong();
                int salt = i % 5 == 0 ? 0 : INPUTS.nextInt();
                int x = INPUTS.nextInt(-2_000_000, 2_000_000), z = INPUTS.nextInt(-2_000_000, 2_000_000);
                StringBuilder sb = new StringBuilder(seed + " " + salt + " " + x + " " + z);
                for (var m : methods) {
                    sb.append(' ').append(m.getSerializedName()).append('=');
                    for (float f : frequencies) {
                        // also a window of chunks, so true and false both show up
                        int hits = 0;
                        for (int d = 0; d < 32; d++) hits = hits << 1 | (m.shouldGenerate(seed, salt, x + d, z - d, f) ? 1 : 0);
                        sb.append(Integer.toHexString(hits)).append(',');
                    }
                }
                w.println(sb);
            }
        }
    }
    // ---- strongholds -----------------------------------------------------------
    /** A biome tag from the game jar's data, nested tags resolved, in file order. */
    static List<String> biomeTag(String tag) {
        try (ZipFile jar = new ZipFile(Path.of(NormalNoise.class.getProtectionDomain().getCodeSource().getLocation().getPath()).toFile())) {
            List<String> out = new ArrayList<>();
            resolveTag(jar, Identifier.parse(tag), out);
            return out;
        } catch (IOException e) {
            throw new RuntimeException(e);
        }
    }

    static void resolveTag(ZipFile jar, Identifier tag, List<String> out) throws IOException {
        ZipEntry e = jar.getEntry("data/" + tag.getNamespace() + "/tags/worldgen/biome/" + tag.getPath() + ".json");
        var json = JsonParser.parseString(new String(jar.getInputStream(e).readAllBytes(), StandardCharsets.UTF_8)).getAsJsonObject();
        for (var v : json.getAsJsonArray("values")) {
            String s = v.isJsonObject() ? v.getAsJsonObject().get("id").getAsString() : v.getAsString();
            if (s.startsWith("#")) resolveTag(jar, Identifier.parse(s.substring(1)), out);
            else if (!out.contains(s)) out.add(s);
        }
    }

    /** Ring positions from the game's own ChunkGeneratorStructureState.generateRingPositions. */
    static void strongholds(HolderLookup.Provider lookup) throws Exception {
        NoiseGeneratorSettings settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.OVERWORLD).value();
        var biomeLookup = lookup.lookupOrThrow(Registries.BIOME);
        var biomeSource = net.minecraft.world.level.biome.MultiNoiseBiomeSource.createFromPreset(
                lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
                        .getOrThrow(net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists.OVERWORLD));
        var set = lookup.lookupOrThrow(Registries.STRUCTURE_SET).getOrThrow(
                ResourceKey.create(Registries.STRUCTURE_SET, Identifier.parse("minecraft:strongholds")));
        var vanilla = (net.minecraft.world.level.levelgen.structure.placement.ConcentricRingsStructurePlacement) set.value().placement();
        // The tag is not bound in a world-less lookup: use the same biomes as a direct set.
        String tag = vanilla.preferredBiomes().unwrapKey().orElseThrow().location().toString();
        List<Holder<Biome>> preferred = new ArrayList<>();
        for (String b : biomeTag(tag)) preferred.add(biomeLookup.getOrThrow(ResourceKey.create(Registries.BIOME, Identifier.parse(b))));
        var placement = new net.minecraft.world.level.levelgen.structure.placement.ConcentricRingsStructurePlacement(
                vanilla.distance(), vanilla.spread(), vanilla.count(), net.minecraft.core.HolderSet.direct(preferred));
        var ctor = net.minecraft.world.level.chunk.ChunkGeneratorStructureState.class.getDeclaredConstructor(
                RandomState.class, net.minecraft.world.level.biome.BiomeSource.class, long.class,
                net.minecraft.world.level.ChunkPos.class, long.class, List.class);
        ctor.setAccessible(true);
        Method generate = net.minecraft.world.level.chunk.ChunkGeneratorStructureState.class.getDeclaredMethod(
                "generateRingPositions", Holder.class, placement.getClass());
        generate.setAccessible(true);
        try (PrintWriter w = writer("strongholds.txt")) {
            for (long seed : List.of(0L, 1L, -4172144997902289642L, INPUTS.nextLong())) {
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                Object state = ctor.newInstance(rs, biomeSource, seed, new net.minecraft.world.level.ChunkPos(0, 0), seed, List.of());
                @SuppressWarnings("unchecked")
                var positions = ((java.util.concurrent.CompletableFuture<List<net.minecraft.world.level.ChunkPos>>)
                        generate.invoke(state, set, placement)).join();
                StringBuilder sb = new StringBuilder(String.valueOf(seed));
                for (var p : positions) sb.append(' ').append(p.x()).append(',').append(p.z());
                w.println(sb);
            }
        }
    }
}
