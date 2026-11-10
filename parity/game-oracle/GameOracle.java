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
        volumeNoise(vanilla);
        finalDensity(lookup, Path.of(args[5]));
        terrain(lookup);
        aquifers(lookup);
        structures(lookup);
        spawnOrigins(lookup);
        netherBiomes(lookup);
        netherStructures(lookup);
        netherTerrain(lookup, Path.of(args[5]));
        ruinedPortals(lookup);
        endIslands();
        endTerrain(lookup, Path.of(args[5]));
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

    /** A private field, looked up in the class and its superclasses. */
    static Object get(Object o, String name) {
        try {
            for (Class<?> k = o.getClass(); k != null; k = k.getSuperclass()) {
                try {
                    Field f = k.getDeclaredField(name);
                    f.setAccessible(true);
                    return f.get(o);
                } catch (NoSuchFieldException e) {
                    // look in the superclass
                }
            }
            throw new NoSuchFieldException(o.getClass().getName() + "." + name);
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
        // The Nether preset, after the overworld one: its biomes and ranges are appended, so the
        // overworld indices do not change.
        var netherParams = MultiNoiseBiomeSourceParameterList.knownPresets().get(MultiNoiseBiomeSourceParameterList.Preset.NETHER);
        StringBuilder netherEntries = new StringBuilder();
        for (var pair : netherParams.values()) {
            Climate.ParameterPoint p = pair.getFirst();
            String biome = pair.getSecond().identifier().toString();
            if (!biomes.contains(biome)) biomes.add(biome);
            netherEntries.append("    ([");
            Climate.Parameter[] ps = {p.temperature(), p.humidity(), p.continentalness(), p.erosion(), p.depth(), p.weirdness()};
            for (int i = 0; i < 6; i++) {
                if (!parameters.contains(ps[i])) parameters.add(ps[i]);
                netherEntries.append(i == 0 ? "" : ", ").append(parameters.indexOf(ps[i]));
            }
            netherEntries.append("], ").append(p.offset()).append(", ").append(biomes.indexOf(biome)).append("),\n");
        }
        // The End's biomes (TheEndBiomeSource), last.
        for (String b : List.of("minecraft:the_end", "minecraft:end_highlands", "minecraft:end_midlands",
                "minecraft:small_end_islands", "minecraft:end_barrens")) if (!biomes.contains(b)) biomes.add(b);
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
        out.append("\n/// The `minecraft:nether` parameter list, in game order, like `OVERWORLD`.\n")
                .append("pub static NETHER: [([u16; 6], i64, u16); ").append(netherParams.values().size()).append("] = [\n")
                .append(netherEntries).append("];\n");

        // The overworld spawn targets (NoiseGeneratorSettings.spawnTarget): climate ranges of
        // the five climate density functions, which are the noise router's own.
        NoiseGeneratorSettings settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.OVERWORLD).value();
        List<String> order = List.of("minecraft:overworld/temperature", "minecraft:overworld/vegetation",
                "minecraft:overworld/continents", "minecraft:overworld/erosion", "minecraft:overworld/ridges");
        out.append("\n/// The overworld spawn targets (`NoiseGeneratorSettings.spawnTarget`): ranges for\n")
                .append("/// temperature, humidity (vegetation), continentalness, erosion and weirdness (ridges).\n")
                .append("pub const SPAWN_TARGETS: [[Parameter; 5]; ").append(settings.spawnTarget().size()).append("] = [\n");
        for (var target : settings.spawnTarget()) {
            Climate.Parameter[] ranges = new Climate.Parameter[5];
            for (var e : target.parameters().entrySet()) {
                String name = e.getKey().unwrapKey().orElseThrow().identifier().toString();
                int i = order.indexOf(name);
                if (i < 0 || ranges[i] != null) throw new IllegalStateException("unexpected spawn target function " + name);
                ranges[i] = e.getValue();
            }
            out.append("    [");
            for (int i = 0; i < 5; i++) {
                if (ranges[i] == null) throw new IllegalStateException("missing spawn target function " + order.get(i));
                out.append(i == 0 ? "" : ", ").append("Parameter::new(").append(ranges[i].min()).append(", ").append(ranges[i].max()).append(")");
            }
            out.append("],\n");
        }
        out.append("];\n");
        Files.writeString(rs, out);
    }

    /** The Nether's compiled final_density (appended to density/generated.rs) and base columns. */
    static void netherTerrain(HolderLookup.Provider lookup, Path densityRs) throws Exception {
        NoiseGeneratorSettings settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.NETHER).value();
        String structure = null;
        try (PrintWriter w = writer("nether_terrain.txt")) {
            for (long seed : List.of(0L, -4172144997902289642L, INPUTS.nextLong())) {
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                List<int[]> columns = new ArrayList<>(List.of(new int[] {0, 0}, new int[] {8, -8}));
                for (int i = 0; i < 40; i++) {
                    int range = i % 2 == 0 ? 3_000 : 1_000_000;
                    columns.add(new int[] {INPUTS.nextInt(-range, range), INPUTS.nextInt(-range, range)});
                }
                for (int[] xz : columns) {
                    Column col = column(rs, settings, xz[0], xz[1]);
                    TreeExport t = exportFinalDensity(lookup, rs, settings, col, seed);
                    String s = String.join("\n", t.nodes) + String.join("\n", t.noises);
                    if (structure == null) {
                        structure = s;
                        String file = Files.readString(densityRs);
                        Files.writeString(densityRs, file + "\n// ---- The Nether -------------------------------------------------------\n\n"
                                + densityTreeSection(t, "NETHER_", "NETHER", "nether", true));
                    } else if (!structure.equals(s)) {
                        throw new IllegalStateException("compiled Nether tree differs between columns or seeds");
                    }
                    col.chunk().close();
                    // The game's own base column (the generator a Nether dimension uses).
                    var settingsHolder = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.NETHER);
                    var biomeSource = net.minecraft.world.level.biome.MultiNoiseBiomeSource.createFromPreset(
                            lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
                                    .getOrThrow(net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists.NETHER));
                    var generator = new net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator(biomeSource, settingsHolder);
                    var height = net.minecraft.world.level.LevelHeightAccessor.create(settings.noiseSettings().minY(), settings.noiseSettings().height());
                    int ws = generator.getBaseHeight(xz[0], xz[1], net.minecraft.world.level.levelgen.Heightmap.Types.WORLD_SURFACE_WG, height, rs);
                    var column = generator.getBaseColumn(xz[0], xz[1], height, rs);
                    StringBuilder blocks = new StringBuilder();
                    for (int y = height.getMinY(); y < height.getMinY() + height.getHeight(); y++) blocks.append(substance(column.getBlock(y), settings));
                    w.println(seed + " " + xz[0] + " " + xz[1] + " " + ws + " " + blocks);
                }
            }
        }
    }

    /** Nether biomes and climate (legacy random source) at random quart positions. */
    static void netherBiomes(HolderLookup.Provider lookup) throws IOException {
        NoiseGeneratorSettings settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.NETHER).value();
        var preset = MultiNoiseBiomeSourceParameterList.knownPresets().get(MultiNoiseBiomeSourceParameterList.Preset.NETHER);
        List<Long> worldSeeds = new ArrayList<>(List.of(0L, 1L, -1L, Long.MIN_VALUE, -4172144997902289642L, 7598295893756170300L));
        for (int i = 0; i < 6; i++) worldSeeds.add(INPUTS.nextLong());
        try (PrintWriter w = writer("nether_biomes.txt")) {
            for (long seed : worldSeeds) {
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                var sampler = rs.createClimateSampler(SamplerContext.builder().build());
                // MultiNoiseBiomeSource.getNoiseBiome is parameters.findValue(sampler.sample(...)).
                // A fresh R-tree per seed: its search starts from nothing, like Chunkscope's.
                var parameters = new Climate.ParameterList<>(preset.values());
                for (int i = 0; i < 400; i++) {
                    int range = i % 2 == 0 ? 2_000 : 500_000;
                    int qx = INPUTS.nextInt(-range, range), qz = INPUTS.nextInt(-range, range);
                    int qy = i % 10 == 0 ? INPUTS.nextInt(-40, 80) : INPUTS.nextInt(0, 32);
                    var target = sampler.sample(qx, qy, qz);
                    var biome = parameters.findValue(target).identifier();
                    w.println(seed + " " + qx + " " + qy + " " + qz + " " + target.temperature() + " " + target.humidity()
                            + " " + target.continentalness() + " " + target.erosion() + " " + target.depth() + " " + target.weirdness()
                            + " " + biome);
                }
            }
        }
    }

    /** NoiseSpawnFinder.findSpawnPosition and NoiseBasedChunkGenerator.getOrigin for many seeds. */
    static void spawnOrigins(HolderLookup.Provider lookup) throws IOException {
        var settingsHolder = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        NoiseGeneratorSettings settings = settingsHolder.value();
        var biomeSource = net.minecraft.world.level.biome.MultiNoiseBiomeSource.createFromPreset(
                lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
                        .getOrThrow(net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists.OVERWORLD));
        var generator = new net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator(biomeSource, settingsHolder);
        List<Long> worldSeeds = new ArrayList<>(List.of(0L, 1L, -1L, 42L, Long.MIN_VALUE, Long.MAX_VALUE,
                -4172144997902289642L, 7598295893756170300L, 485913639L));
        for (int i = 0; i < 400; i++) worldSeeds.add(INPUTS.nextLong());
        try (PrintWriter w = writer("spawn_origin.txt")) {
            for (long seed : worldSeeds) {
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                var samplers = rs.samplersWithContext(SamplerContext.builder().enableCaches().build());
                var pos = net.minecraft.world.level.levelgen.NoiseSpawnFinder.findSpawnPosition(settings.spawnTarget(), samplers);
                var origin = generator.getOrigin(rs);
                w.println(seed + " " + pos.getX() + " " + pos.getZ() + " " + origin.x() + " " + origin.z());
            }
        }
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
        out.append("use super::placement::*;\nuse super::validate::*;\nuse crate::terrain::Heightmap;\n\n");
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
        writeStructures(lookup, sets, out);
        Files.writeString(rs, out);
    }

    static String verticalAnchor(net.minecraft.world.level.levelgen.VerticalAnchor a) {
        if (a instanceof net.minecraft.world.level.levelgen.VerticalAnchor.Absolute v) return "VerticalAnchor::Absolute(" + v.y() + ")";
        if (a instanceof net.minecraft.world.level.levelgen.VerticalAnchor.AboveBottom v) return "VerticalAnchor::AboveBottom(" + v.offset() + ")";
        if (a instanceof net.minecraft.world.level.levelgen.VerticalAnchor.BelowTop v) return "VerticalAnchor::BelowTop(" + v.offset() + ")";
        throw new IllegalStateException("unsupported anchor " + a);
    }

    static String anchor(net.minecraft.world.level.levelgen.VerticalAnchor a) {
        if (a instanceof net.minecraft.world.level.levelgen.VerticalAnchor.Absolute abs) return Integer.toString(abs.y());
        throw new IllegalStateException("unsupported anchor " + a);
    }

    /** The structures of every structure set: biomes, and what their generation point needs. */
    static void writeStructures(HolderLookup.Provider lookup,
            List<net.minecraft.core.Holder.Reference<net.minecraft.world.level.levelgen.structure.StructureSet>> sets,
            StringBuilder out) {
        java.util.Set<String> skip = java.util.Set.of("minecraft:strongholds", "minecraft:end_cities");
        List<String> names = new ArrayList<>();
        for (var ref : sets) {
            if (skip.contains(ref.key().identifier().toString())) continue;
            for (var e : ref.value().structures()) names.add(e.structure().unwrapKey().orElseThrow().identifier().toString());
        }
        java.util.Collections.sort(names);
        var registry = lookup.lookupOrThrow(Registries.STRUCTURE);
        Map<String, Object[]> startPools = new java.util.TreeMap<>();
        out.append("\n/// The structures of the overworld and Nether structure sets (strongholds excepted), sorted by name.\n");
        out.append("pub static STRUCTURES: [Structure; ").append(names.size()).append("] = [\n");
        for (String name : names) {
            var structure = registry.getOrThrow(ResourceKey.create(Registries.STRUCTURE, Identifier.parse(name))).value();
            String tag = ((net.minecraft.core.HolderSet.Named<Biome>) structure.biomes()).key().location().toString();
            StringBuilder biomes = new StringBuilder();
            for (String b : biomeTag(tag)) biomes.append(biomes.length() == 0 ? "" : ", ").append('"').append(b).append('"');
            out.append("    Structure {\n        name: \"").append(name).append("\",\n        biomes: &[").append(biomes).append("],\n        kind: ");
            String kind;
            if (structure instanceof net.minecraft.world.level.levelgen.structure.structures.JigsawStructure j) {
                Object height = get(j, "startHeight");
                String start;
                if (height instanceof net.minecraft.world.level.levelgen.heightproviders.ConstantHeight c) {
                    start = "HeightProvider::Constant(" + anchor(c.getValue()) + ")";
                } else if (height instanceof net.minecraft.world.level.levelgen.heightproviders.UniformHeight u) {
                    start = "HeightProvider::Uniform { min_inclusive: " + anchor((net.minecraft.world.level.levelgen.VerticalAnchor) get(u, "minInclusive"))
                            + ", max_inclusive: " + anchor((net.minecraft.world.level.levelgen.VerticalAnchor) get(u, "maxInclusive")) + " }";
                } else throw new IllegalStateException("unsupported height " + height);
                @SuppressWarnings("unchecked")
                var project = (java.util.Optional<net.minecraft.world.level.levelgen.Heightmap.Types>) get(j, "projectStartToHeightmap");
                @SuppressWarnings("unchecked")
                var jigsawName = (java.util.Optional<Identifier>) get(j, "startJigsawName");
                var padding = (net.minecraft.world.level.levelgen.structure.pools.DimensionPadding) get(j, "dimensionPadding");
                @SuppressWarnings("unchecked")
                var pool = (Holder<net.minecraft.world.level.levelgen.structure.pools.StructureTemplatePool>) get(j, "startPool");
                kind = "StructureKind::Jigsaw {\n            start_pool: \"" + pool.unwrapKey().orElseThrow().identifier()
                        + "\",\n            start_jigsaw_name: " + jigsawName.map(n -> "Some(\"" + n + "\")").orElse("None")
                        + ",\n            start_height: " + start
                        + ",\n            project_start_to_heightmap: " + project.map(GameOracle::heightmap).orElse("None")
                        + ",\n            dimension_padding: " + (padding == net.minecraft.world.level.levelgen.structure.pools.DimensionPadding.ZERO
                                ? "None" : "Some((" + padding.bottom() + ", " + padding.top() + "))") + ",\n        }";
                startPools.put(pool.unwrapKey().orElseThrow().identifier().toString(), new Object[] {pool.value(), jigsawName.orElse(null)});
                @SuppressWarnings("unchecked")
                var aliases = (List<net.minecraft.world.level.levelgen.structure.pools.alias.PoolAliasBinding>) get(j, "poolAliases");
                // The start pool goes through the structure's pool aliases; none of them rename it.
                for (var alias : aliases) {
                    alias.forEachResolved(new LegacyRandomSource(0), (from, to) -> {
                        if (from.equals(pool.unwrapKey().orElseThrow())) throw new IllegalStateException("aliased start pool " + from);
                    });
                }
            } else if (structure instanceof net.minecraft.world.level.levelgen.structure.SinglePieceStructure sp) {
                kind = "StructureKind::SinglePiece { width: " + get(sp, "width") + ", depth: " + get(sp, "depth") + " }";
            } else if (structure instanceof net.minecraft.world.level.levelgen.structure.structures.ShipwreckStructure sw) {
                kind = "StructureKind::OnTopOfChunkCenter(" + ((boolean) get(sw, "isBeached") ? "Heightmap::WorldSurface" : "Heightmap::OceanFloor") + ")";
            } else if (structure instanceof net.minecraft.world.level.levelgen.structure.structures.SwampHutStructure
                    || structure instanceof net.minecraft.world.level.levelgen.structure.structures.IglooStructure) {
                kind = "StructureKind::OnTopOfChunkCenter(Heightmap::WorldSurface)";
            } else if (structure instanceof net.minecraft.world.level.levelgen.structure.structures.OceanRuinStructure
                    || structure instanceof net.minecraft.world.level.levelgen.structure.structures.BuriedTreasureStructure) {
                kind = "StructureKind::OnTopOfChunkCenter(Heightmap::OceanFloor)";
            } else if (structure instanceof net.minecraft.world.level.levelgen.structure.structures.NetherFossilStructure f) {
                var u = (net.minecraft.world.level.levelgen.heightproviders.UniformHeight) get(f, "height");
                kind = "StructureKind::NetherFossil {\n            min_inclusive: " + verticalAnchor((net.minecraft.world.level.levelgen.VerticalAnchor) get(u, "minInclusive"))
                        + ",\n            max_inclusive: " + verticalAnchor((net.minecraft.world.level.levelgen.VerticalAnchor) get(u, "maxInclusive")) + ",\n        }";
            } else if (structure instanceof net.minecraft.world.level.levelgen.structure.structures.RuinedPortalStructure rp) {
                @SuppressWarnings("unchecked")
                var setups = (List<net.minecraft.world.level.levelgen.structure.structures.RuinedPortalStructure.Setup>) get(rp, "setups");
                StringBuilder list = new StringBuilder();
                for (var setup : setups) {
                    String placement = java.util.Arrays.stream(setup.placement().name().split("_"))
                            .map(w -> w.charAt(0) + w.substring(1).toLowerCase()).collect(java.util.stream.Collectors.joining());
                    list.append("\n            PortalSetup {\n                placement: PortalPlacement::").append(placement)
                            .append(",\n                weight: ").append(rustF32(setup.weight()))
                            .append(",\n                air_pocket_probability: ").append(rustF32(setup.airPocketProbability()))
                            .append(",\n            },");
                }
                kind = "StructureKind::RuinedPortal(&[" + list + "\n        ])";
            } else if (structure instanceof net.minecraft.world.level.levelgen.structure.structures.NetherFortressStructure) {
                kind = "StructureKind::NetherFortress";
            } else if (structure instanceof net.minecraft.world.level.levelgen.structure.structures.WoodlandMansionStructure) {
                kind = "StructureKind::WoodlandMansion";
            } else if (structure instanceof net.minecraft.world.level.levelgen.structure.structures.OceanMonumentStructure) {
                kind = "StructureKind::OceanMonument";
            } else {
                kind = "StructureKind::NotImplemented";
            }
            out.append(kind).append(",\n    },\n");
        }
        out.append("];\n");
        StringBuilder surrounding = new StringBuilder();
        for (String b : biomeTag("minecraft:required_ocean_monument_surrounding")) surrounding.append(surrounding.length() == 0 ? "" : ", ").append('"').append(b).append('"');
        writeStartPools(lookup, startPools, out);
        writePortalTemplates(lookup, out);
        out.append("\n/// `#minecraft:required_ocean_monument_surrounding`.\npub static OCEAN_MONUMENT_SURROUNDING: &[&str] = &[")
                .append(surrounding).append("];\n");
    }

    /** The sizes of the ruined portal templates, in the game's order. */
    static void writePortalTemplates(HolderLookup.Provider lookup, StringBuilder out) {
        try {
            var templates = templateManager(lookup);
            for (String[] f : new String[][] {{"STRUCTURE_LOCATION_PORTALS", "RUINED_PORTALS"}, {"STRUCTURE_LOCATION_GIANT_PORTALS", "GIANT_RUINED_PORTALS"}}) {
                Field field = net.minecraft.world.level.levelgen.structure.structures.RuinedPortalStructure.class.getDeclaredField(f[0]);
                field.setAccessible(true);
                String[] names = (String[]) field.get(null);
                out.append("\n/// `RuinedPortalStructure.").append(f[0]).append("`: template sizes (x, y, z).\n");
                out.append("pub static ").append(f[1]).append(": [[i32; 3]; ").append(names.length).append("] = [\n");
                for (String name : names) {
                    var size = templates.getOrCreate(Identifier.withDefaultNamespace(name)).getSize();
                    out.append("    [").append(size.getX()).append(", ").append(size.getY()).append(", ").append(size.getZ())
                            .append("], // ").append(name).append('\n');
                }
                out.append("];\n");
            }
        } catch (ReflectiveOperationException | IOException e) {
            throw new RuntimeException(e);
        }
    }

    /** Start pools of the jigsaw structures: what their start piece's bounding box and anchor need. */
    static void writeStartPools(HolderLookup.Provider lookup, Map<String, Object[]> pools, StringBuilder out) {
        try {
            var templates = templateManager(lookup);
            var rotations = net.minecraft.world.level.block.Rotation.values();
            out.append("\n/// Start pools of the jigsaw structures, sorted by name. `elements` are the distinct\n");
            out.append("/// elements; `templates` is the game's weighted list, as indices into `elements`.\n");
            out.append("pub static START_POOLS: [StartPool; ").append(pools.size()).append("] = [\n");
            for (var e : pools.entrySet()) {
                var pool = (net.minecraft.world.level.levelgen.structure.pools.StructureTemplatePool) e.getValue()[0];
                var anchorName = (Identifier) e.getValue()[1];
                @SuppressWarnings("unchecked")
                var weighted = (List<net.minecraft.world.level.levelgen.structure.pools.StructurePoolElement>) get(pool, "templates");
                List<net.minecraft.world.level.levelgen.structure.pools.StructurePoolElement> distinct = new ArrayList<>();
                StringBuilder indices = new StringBuilder();
                for (var el : weighted) {
                    int i = distinct.indexOf(el);
                    if (i < 0) {
                        i = distinct.size();
                        distinct.add(el);
                    }
                    indices.append(indices.length() == 0 ? "" : ", ").append(i);
                }
                out.append("    StartPool {\n        name: \"").append(e.getKey()).append("\",\n        elements: &[\n");
                for (var el : distinct) {
                    if (el == net.minecraft.world.level.levelgen.structure.pools.EmptyPoolElement.INSTANCE) {
                        out.append("            None,\n");
                        continue;
                    }
                    StringBuilder boxes = new StringBuilder(), anchors = new StringBuilder();
                    boolean anchored = true;
                    for (var rotation : rotations) {
                        var box = el.getBoundingBox(templates, net.minecraft.core.BlockPos.ZERO, rotation);
                        boxes.append(boxes.length() == 0 ? "" : ", ").append("[").append(box.minX()).append(", ").append(box.minY())
                                .append(", ").append(box.minZ()).append(", ").append(box.maxX()).append(", ").append(box.maxY())
                                .append(", ").append(box.maxZ()).append("]");
                        if (anchorName != null) {
                            var matches = el.getShuffledJigsawBlocks(templates, net.minecraft.core.BlockPos.ZERO, rotation,
                                    new LegacyRandomSource(0)).stream().filter(j -> j.name().equals(anchorName)).toList();
                            if (matches.size() > 1) throw new IllegalStateException("several start jigsaws in " + el);
                            if (matches.isEmpty()) {
                                anchored = false;
                            } else {
                                var pos = matches.get(0).pos();
                                anchors.append(anchors.length() == 0 ? "" : ", ").append("[").append(pos.getX()).append(", ")
                                        .append(pos.getY()).append(", ").append(pos.getZ()).append("]");
                            }
                        }
                    }
                    out.append("            Some(PoolElement {\n                boxes: [").append(boxes).append("],\n")
                            .append("                ground_level_delta: ").append(el.getGroundLevelDelta()).append(",\n")
                            .append("                anchor: ").append(anchorName == null ? "Anchor::Origin"
                                    : anchored ? "Anchor::Jigsaw([" + anchors + "])" : "Anchor::Missing").append(",\n            }),\n");
                }
                out.append("        ],\n        templates: &[").append(indices).append("],\n    },\n");
            }
            out.append("];\n");
        } catch (IOException ex) {
            throw new RuntimeException(ex);
        }
    }

    static String heightmap(net.minecraft.world.level.levelgen.Heightmap.Types t) {
        return switch (t) {
            case WORLD_SURFACE_WG -> "Some(Heightmap::WorldSurface)";
            case OCEAN_FLOOR_WG -> "Some(Heightmap::OceanFloor)";
            default -> throw new IllegalStateException("unsupported heightmap " + t);
        };
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
        resolveTag(jar, "worldgen/biome", tag, out);
    }

    static void resolveTag(ZipFile jar, String registry, Identifier tag, List<String> out) throws IOException {
        ZipEntry e = jar.getEntry("data/" + tag.getNamespace() + "/tags/" + registry + "/" + tag.getPath() + ".json");
        var json = JsonParser.parseString(new String(jar.getInputStream(e).readAllBytes(), StandardCharsets.UTF_8)).getAsJsonObject();
        for (var v : json.getAsJsonArray("values")) {
            String s = v.isJsonObject() ? v.getAsJsonObject().get("id").getAsString() : v.getAsString();
            if (s.startsWith("#")) resolveTag(jar, registry, Identifier.parse(s.substring(1)), out);
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
    // ---- volume-mode noise (terrain path) ----------------------------------------
    static long fnv(net.minecraft.world.level.levelgen.densityfunction.DensityBuffer b, int n) {
        long h = 0xcbf29ce484222325L;
        for (int i = 0; i < n; i++) {
            h ^= Integer.toUnsignedLong(Float.floatToRawIntBits(b.get(i)));
            h *= 0x100000001b3L;
        }
        return h;
    }

    /** NoiseStack.addToVolume for every vanilla noise over several volume shapes. */
    static void volumeNoise(Map<String, NormalNoise> vanilla) throws IOException {
        record Shape(String name, int[] v, double xz, double y) {}
        List<Shape> shapes = new ArrayList<>(List.of(
                new Shape("column", new int[] {1, 384, 1, 0, -64, 0, 1, 1, 1}, 1.0, 1.0),
                new Shape("cells", new int[] {5, 49, 5, -16, -64, 32, 4, 8, 4}, 0.25, 0.125),
                new Shape("slice", new int[] {16, 1, 16, 1000, 0, -2000, 1, 1, 1}, 0.25, 0.0),
                new Shape("far", new int[] {3, 20, 3, 29_999_990, 300, -29_999_990, 2, 3, 2}, 1.0, 0.5)));
        for (int i = 0; i < 4; i++) {
            int x = INPUTS.nextInt(-2_000_000, 2_000_000), z = INPUTS.nextInt(-2_000_000, 2_000_000);
            shapes.add(new Shape("random" + i, new int[] {INPUTS.nextInt(1, 6), INPUTS.nextInt(1, 40), INPUTS.nextInt(1, 6), x,
                    INPUTS.nextInt(-64, 300), z, INPUTS.nextInt(1, 5), INPUTS.nextInt(1, 9), INPUTS.nextInt(1, 5)},
                    INPUTS.nextDouble() * 2, INPUTS.nextDouble() * 2));
        }
        try (PrintWriter w = writer("volume_noise.txt")) {
            for (Shape s : shapes) {
                StringBuilder sb = new StringBuilder("shape " + s.name());
                for (int v : s.v()) sb.append(' ').append(v);
                sb.append(' ').append(bits(s.xz())).append(' ').append(bits(s.y()));
                w.println(sb);
            }
            for (long seed : List.of(0L, -4172144997902289642L, INPUTS.nextLong())) {
                PositionalRandomFactory factory = new XoroshiroRandomSource(seed).forkPositional();
                for (var e : vanilla.entrySet()) {
                    Noise n = e.getValue().create(factory.fromHashOf(e.getKey()));
                    StringBuilder sb = new StringBuilder(seed + " " + e.getKey());
                    for (Shape s : shapes) {
                        int[] v = s.v();
                        var volume = new net.minecraft.world.level.levelgen.densityfunction.DensityVolume(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8]);
                        int size = v[0] * v[1] * v[2];
                        var buffer = net.minecraft.world.level.levelgen.densityfunction.DensityBuffer.createUnpooled(size);
                        buffer.fill(0.0f);
                        n.addToVolume(buffer, volume, s.xz(), s.y(), 1.0f);
                        n.addToVolume(buffer, volume, s.xz(), s.y(), -0.75f); // accumulation on top
                        sb.append(' ').append(s.name()).append('=').append(Long.toHexString(fnv(buffer, size)))
                                .append(',').append(bits(buffer.get(0))).append(',').append(bits(buffer.get(size / 2)))
                                .append(',').append(bits(buffer.get(size - 1)));
                    }
                    w.println(sb);
                }
            }
        }
    }
    // ---- final_density: compiled tree export + per-node volume fixtures -----------
    static final String DF = "net.minecraft.world.level.levelgen.densityfunction.";

    static Object fieldAny(Object o, String name) {
        for (Class<?> k = o.getClass(); k != null; k = k.getSuperclass()) {
            try {
                Field f = k.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(o);
            } catch (NoSuchFieldException e) {
                // superclass
            } catch (ReflectiveOperationException e) {
                throw new RuntimeException(e);
            }
        }
        throw new IllegalStateException("no field " + name + " in " + o.getClass().getName());
    }

    /** The NoiseChunk getBaseHeight builds for a column, with its bound final_density sampler. */
    record Column(net.minecraft.world.level.levelgen.NoiseChunk chunk, DensitySampler.Bound bound,
            net.minecraft.world.level.levelgen.densityfunction.DensityVolume volume) {}

    static Column column(RandomState rs, NoiseGeneratorSettings settings, int x, int z) throws Exception {
        Method picker = net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator.class.getDeclaredMethod("createFluidPicker", NoiseGeneratorSettings.class);
        picker.setAccessible(true);
        var ns = settings.noiseSettings();
        var volume = new net.minecraft.world.level.levelgen.densityfunction.DensityVolume(1, ns.height(), 1, x, ns.minY(), z);
        var chunk = new net.minecraft.world.level.levelgen.NoiseChunk(rs, null, settings,
                (net.minecraft.world.level.levelgen.Aquifer.FluidPicker) picker.invoke(null, settings),
                net.minecraft.world.level.levelgen.blending.Blender.empty(), volume);
        return new Column(chunk, chunk.cachingSamplers().get(settings.noiseRouter().finalDensity()), volume);
    }

    static final class TreeExport {
        final Map<Object, Integer> ids = new java.util.IdentityHashMap<>();
        final List<String> nodes = new ArrayList<>();
        final Map<Object, Integer> noiseIds = new java.util.IdentityHashMap<>();
        final List<String> noises = new ArrayList<>();
        final Map<Object, String> noiseNames;
        final Map<Object, String> blendedStacks;
        String blendedParams;
        final Map<String, Integer> roots = new LinkedHashMap<>();

        TreeExport(Map<Object, String> noiseNames, Map<Object, String> blendedStacks) {
            this.noiseNames = noiseNames;
            this.blendedStacks = blendedStacks;
        }

        int noise(Object n) {
            return noiseIds.computeIfAbsent(n, k -> {
                String src = noiseNames.get(k) != null ? "NoiseSource::Vanilla(\"" + noiseNames.get(k) + "\")" : blendedStacks.get(k);
                if (src == null) throw new IllegalStateException("unknown noise " + k);
                noises.add(src);
                return noises.size() - 1;
            });
        }

        /** Post-order: children get ids first, so the root is last. */
        int node(Object s) {
            Integer known = ids.get(s);
            if (known != null) return known;
            String cls = s.getClass().getName().replace(DF, "");
            String def = switch (cls) {
                case "generator.EndIslandFunction$Sampler" -> "Node::EndIslands";
                case "generator.DistanceToPointFunction$Sampler" -> {
                    String metric = java.util.Arrays.stream(fieldAny(s, "metric").toString().split("_"))
                            .map(w -> w.charAt(0) + w.substring(1).toLowerCase()).collect(java.util.stream.Collectors.joining());
                    yield "Node::DistanceToPoint { x: " + fieldAny(s, "x") + ", y: " + fieldAny(s, "y") + ", z: " + fieldAny(s, "z")
                            + ", metric: DistanceMetric::" + metric + " }";
                }
                case "generator.ConstantFunction$Sampler" -> "Node::Constant(" + rustF32Any((float) fieldAny(s, "value")) + ")";
                case "generator.NoiseFunction$Sampler" -> "Node::Noise { noise: " + noise(fieldAny(s, "noise")) + ", xz_scale: "
                        + rustF64((double) fieldAny(s, "xzScale")) + ", y_scale: " + rustF64((double) fieldAny(s, "yScale")) + " }";
                case "generator.NoiseFunction$ShiftedXzSampler" -> "Node::ShiftedXz { shift_x: " + node(fieldAny(s, "shiftX")) + ", shift_z: "
                        + node(fieldAny(s, "shiftZ")) + ", noise: " + noise(fieldAny(s, "noise")) + ", xz_scale: "
                        + rustF64((double) fieldAny(s, "xzScale")) + ", y_scale: " + rustF64((double) fieldAny(s, "yScale")) + " }";
                case "generator.ShiftNoiseFunction$ShiftB$1" -> "Node::ShiftB { noise: " + noise(fieldAny(s, "val$noise")) + " }";
                case "generator.GradientFunction$ClampedSampler" -> {
                    String axis = fieldAny(s, "axis").toString();
                    yield "Node::ClampedGradient { axis: Axis::" + axis.toUpperCase() + ", from_coordinate: " + fieldAny(s, "fromCoordinate")
                            + ", min_coordinate: " + fieldAny(s, "minCoordinate") + ", max_coordinate: " + fieldAny(s, "maxCoordinate")
                            + ", from_value: " + rustF32Any((float) fieldAny(s, "fromValue")) + ", coordinate_factor: "
                            + rustF32Any((float) fieldAny(s, "coordinateFactor")) + " }";
                }
                case "op.BinaryFunction$AddSampler" -> "Node::Add(" + node(fieldAny(s, "left")) + ", " + node(fieldAny(s, "right")) + ")";
                case "op.BinaryFunction$MulSampler" -> "Node::Mul(" + node(fieldAny(s, "left")) + ", " + node(fieldAny(s, "right")) + ")";
                case "op.BinaryFunction$MinSampler" -> "Node::Min { left: " + node(fieldAny(s, "left")) + ", right: " + node(fieldAny(s, "right"))
                        + ", right_min_value: " + rustF32Any((float) fieldAny(s, "rightMinValue")) + " }";
                case "op.BinaryFunction$MaxSampler" -> "Node::Max { left: " + node(fieldAny(s, "left")) + ", right: " + node(fieldAny(s, "right"))
                        + ", right_max_value: " + rustF32Any((float) fieldAny(s, "rightMaxValue")) + " }";
                case "op.BinaryFunction$SubSampler" -> "Node::Sub(" + node(fieldAny(s, "left")) + ", " + node(fieldAny(s, "right")) + ")";
                case "op.BinaryFunction$ConstSubSampler" -> "Node::ConstSub(" + rustF32Any((float) fieldAny(s, "left")) + ", " + node(fieldAny(s, "right")) + ")";
                case "op.BinaryFunction$ConstDivSampler" -> "Node::ConstDiv(" + rustF32Any((float) fieldAny(s, "left")) + ", " + node(fieldAny(s, "right")) + ")";
                case "op.BinaryFunction$ConstMinSampler" -> "Node::ConstMin(" + node(fieldAny(s, "left")) + ", " + rustF32Any((float) fieldAny(s, "right")) + ")";
                case "op.BinaryFunction$ConstMaxSampler" -> "Node::ConstMax(" + node(fieldAny(s, "left")) + ", " + rustF32Any((float) fieldAny(s, "right")) + ")";
                case "op.FindTopSurfaceFunction$Sampler" -> "Node::FindTopSurface { density: " + node(fieldAny(s, "density")) + ", upper_bound: "
                        + node(fieldAny(s, "upperBound")) + ", lower_bound: " + fieldAny(s, "lowerBound") + ", cell_height: " + fieldAny(s, "cellHeight") + " }";
                case "op.BinaryFunction$ConstAddSampler" -> "Node::ConstAdd(" + node(fieldAny(s, "left")) + ", " + rustF32Any((float) fieldAny(s, "right")) + ")";
                case "op.BinaryFunction$ConstMulSampler" -> "Node::ConstMul(" + node(fieldAny(s, "left")) + ", " + rustF32Any((float) fieldAny(s, "right")) + ")";
                case "op.UnaryFunction$AbsSampler" -> "Node::Abs(" + node(fieldAny(s, "input")) + ")";
                case "op.UnaryFunction$CubeSampler" -> "Node::Cube(" + node(fieldAny(s, "input")) + ")";
                case "op.UnaryFunction$SquareSampler" -> "Node::Square(" + node(fieldAny(s, "input")) + ")";
                case "op.UnaryFunction$SqueezeSampler" -> "Node::Squeeze(" + node(fieldAny(s, "input")) + ")";
                case "op.UnaryFunction$LeakyReLUSampler" -> "Node::LeakyRelu { input: " + node(fieldAny(s, "input")) + ", negative_factor: "
                        + rustF32Any((float) fieldAny(s, "negativeFactor")) + " }";
                case "op.ClampFunction$Sampler" -> "Node::Clamp { input: " + node(fieldAny(s, "input")) + ", min: "
                        + rustF32Any((float) fieldAny(s, "min")) + ", max: " + rustF32Any((float) fieldAny(s, "max")) + " }";
                case "op.LerpFunction$Sampler" -> "Node::Lerp { alpha: " + node(fieldAny(s, "alpha")) + ", first: " + node(fieldAny(s, "first"))
                        + ", second: " + node(fieldAny(s, "second")) + " }";
                case "op.LerpFunction$ConstFirstSampler" -> "Node::LerpConstFirst { alpha: " + node(fieldAny(s, "alpha")) + ", first: "
                        + rustF32Any((float) fieldAny(s, "first")) + ", second: " + node(fieldAny(s, "second")) + " }";
                case "op.RangeChoiceFunction$Sampler" -> "Node::RangeChoice { input: " + node(fieldAny(s, "input")) + ", min_inclusive: "
                        + rustF32Any((float) fieldAny(s, "minInclusive")) + ", max_exclusive: " + rustF32Any((float) fieldAny(s, "maxExclusive"))
                        + ", when_in_range: " + node(fieldAny(s, "whenInRange")) + ", when_out_of_range: " + node(fieldAny(s, "whenOutOfRange")) + " }";
                case "op.IntervalSelectFunction$Sampler" -> {
                    float[] t = (float[]) fieldAny(s, "thresholds");
                    Object[] ss = (Object[]) fieldAny(s, "samplers");
                    StringBuilder b = new StringBuilder("Node::IntervalSelect { input: " + node(fieldAny(s, "input")) + ", thresholds: &[");
                    for (int i = 0; i < t.length; i++) b.append(i == 0 ? "" : ", ").append(rustF32Any(t[i]));
                    b.append("], samplers: &[");
                    for (int i = 0; i < ss.length; i++) b.append(i == 0 ? "" : ", ").append(node(ss[i]));
                    yield b.append("] }").toString();
                }
                case "op.InterpolatedFunction$Sampler" -> "Node::Interpolated { input: " + node(fieldAny(s, "input")) + ", cell_size_xz: "
                        + fieldAny(s, "cellSizeXz") + ", cell_size_y: " + fieldAny(s, "cellSizeY") + ", cell_size_xz_inv: "
                        + rustF32Any((float) fieldAny(s, "cellSizeXzInv")) + ", cell_size_y_inv: " + rustF32Any((float) fieldAny(s, "cellSizeYInv")) + " }";
                case "op.SliceFunction$YSampler" -> "Node::SliceY { input: " + node(fieldAny(s, "input")) + ", y: " + fieldAny(s, "y") + " }";
                case "CachingDensitySampler" -> "Node::Cache { input: " + node(fieldAny(s, "input")) + ", id: " + fieldAny(s, "id") + " }";
                case "op.BlendDensityFunction$Sampler" -> "Node::Blend(" + node(fieldAny(s, "input")) + ")";
                case "ContextBoundSampler" -> "Node::ContextBound { key: \"" + fieldAny(fieldAny(s, "key"), "name") + "\", fallback: "
                        + node(fieldAny(s, "fallbackSampler")) + " }";
                case "op.SplineFunction$Sampler" -> {
                    Map<Integer, Integer> coordinates = new java.util.TreeMap<>();
                    String spline = spline(fieldAny(s, "sampler"), coordinates);
                    StringBuilder b = new StringBuilder("Node::Spline { coordinates: &[");
                    int expected = 0;
                    for (var e : coordinates.entrySet()) {
                        if (e.getKey() != expected++) throw new IllegalStateException("sparse spline coordinates");
                        b.append(e.getKey() == 0 ? "" : ", ").append(e.getValue());
                    }
                    yield b.append("], spline: &").append(spline).append(" }").toString();
                }
                default -> throw new IllegalStateException("unsupported sampler " + cls);
            };
            int id = nodes.size();
            ids.put(s, id);
            nodes.add(def);
            return id;
        }

        String spline(Object fn, Map<Integer, Integer> coordinates) {
            Object mp;
            try {
                mp = fieldAny(fn, "val$multipoint");
            } catch (IllegalStateException e) {
                throw new IllegalStateException("unsupported spline sampler " + fn.getClass().getName());
            }
            return multipoint(mp, coordinates);
        }

        String multipoint(Object mp, Map<Integer, Integer> coordinates) {
            Object coord = call(mp, "coordinate");
            int index = (int) fieldAny(coord, "index");
            coordinates.put(index, node(fieldAny(coord, "sampler")));
            float[] loc = (float[]) call(mp, "locations");
            float[] der = (float[]) call(mp, "derivatives");
            StringBuilder b = new StringBuilder("Spline::Multipoint(&Multipoint { coordinate: " + index + ", locations: &[");
            for (int i = 0; i < loc.length; i++) b.append(i == 0 ? "" : ", ").append(rustF32Any(loc[i]));
            b.append("], derivatives: &[");
            for (int i = 0; i < der.length; i++) b.append(i == 0 ? "" : ", ").append(rustF32Any(der[i]));
            b.append("], values: &[");
            boolean first = true;
            for (Object v : (List<?>) call(mp, "values")) {
                b.append(first ? "" : ", ");
                first = false;
                if (v instanceof CubicSpline.Constant<?> c) b.append("Spline::Constant(").append(rustF32Any(c.value())).append(")");
                else b.append(multipoint(v, coordinates));
            }
            return b.append("] })").toString();
        }
    }

    static String rustF32Any(float f) {
        if (Float.isNaN(f)) return "f32::NAN";
        if (Float.isInfinite(f)) return f > 0 ? "f32::INFINITY" : "f32::NEG_INFINITY";
        return Float.toString(f).replace("E", "e");
    }

    static TreeExport exportFinalDensity(HolderLookup.Provider lookup, RandomState rs, NoiseGeneratorSettings settings, Column col, long seed) throws Exception {
        Map<Object, String> names = new java.util.IdentityHashMap<>();
        @SuppressWarnings("unchecked")
        var instances = (Map<ResourceKey<?>, Object>) fieldAny(rs, "noiseInstances");
        // instantiate every vanilla noise so the compiled tree's instances are named
        for (var e : lookup.lookupOrThrow(Registries.NOISE).listElementIds().toList()) rs.getOrCreateNoise(e);
        instances.forEach((k, v) -> names.put(v, k.identifier().toString()));
        // the three smeared stacks of old_blended_noise, recreated to match by offset
        var blended = find(settings.noiseRouter().finalDensity(), net.minecraft.world.level.levelgen.synth.BlendedNoise.class);
        var factory = (PositionalRandomFactory) fieldAny(rs, "random");
        // RandomState's wiring: with the legacy random source, the blended noise is seeded
        // with new LegacyRandomSource(seed) instead of the positional factory.
        var fresh = blended.createFbmSet(settings.useLegacyRandomSource() ? new LegacyRandomSource(seed)
                : factory.fromHashOf(net.minecraft.world.level.levelgen.synth.BlendedNoise.NOISE_SEED));
        Map<Object, String> stacks = new java.util.IdentityHashMap<>();
        java.util.Set<Object> seen = java.util.Collections.newSetFromMap(new java.util.IdentityHashMap<>());
        java.util.ArrayDeque<Object> queue = new java.util.ArrayDeque<>(List.of(col.bound().sampler()));
        while (!queue.isEmpty()) {
            Object o = queue.poll();
            if (o == null || !seen.add(o) || !o.getClass().getName().startsWith("net.minecraft")) continue;
            if (o.getClass().getSimpleName().equals("NoiseStack$SmearedPerlin") || o.getClass().getName().endsWith("NoiseStack$SmearedPerlin")) {
                double off = firstOffsetX(o);
                String which = off == firstOffsetX(fresh.minLimitNoise()) ? "NoiseSource::BlendedMinLimit"
                        : off == firstOffsetX(fresh.maxLimitNoise()) ? "NoiseSource::BlendedMaxLimit"
                        : off == firstOffsetX(fresh.mainNoise()) ? "NoiseSource::BlendedMain" : null;
                if (which == null) throw new IllegalStateException("unmatched smeared stack");
                stacks.put(o, which);
                continue;
            }
            for (Class<?> k = o.getClass(); k != null && k.getName().startsWith("net.minecraft"); k = k.getSuperclass())
                for (Field f : k.getDeclaredFields()) {
                    if (java.lang.reflect.Modifier.isStatic(f.getModifiers()) || f.getType().isPrimitive()) continue;
                    f.setAccessible(true);
                    Object v = f.get(o);
                    if (v instanceof Object[] arr) queue.addAll(java.util.Arrays.asList(arr));
                    else if (v instanceof Iterable<?> it) it.forEach(queue::add);
                    else queue.add(v);
                }
        }
        TreeExport t = new TreeExport(names, stacks);
        // final_density first (keeps its node ids stable), then the aquifer's own functions
        t.roots.put("FINAL_DENSITY", t.node(col.bound().sampler()));
        Object aquifer = col.chunk().aquifer();
        if (settings.aquifers().isPresent()) for (String[] f : new String[][] {{"AQUIFER_BARRIER", "barrierNoise"}, {"AQUIFER_FLOODEDNESS", "fluidLevelFloodednessNoise"},
                {"AQUIFER_SPREAD", "fluidLevelSpreadNoise"}, {"AQUIFER_LAVA", "lavaNoise"}, {"AQUIFER_EXCLUSION", "exclusion"},
                {"AQUIFER_SURFACE_LEVEL", "surfaceLevel"}}) {
            var bound = (DensitySampler.Bound) fieldAny(aquifer, f[1]);
            if (bound.context() != col.bound().context()) throw new IllegalStateException("aquifer sampler context differs");
            t.roots.put(f[0], t.node(bound.sampler()));
        }
        t.blendedParams = "BlendedNoiseParams { xz_scale: " + rustF64(blended.xzScale()) + ", y_scale: " + rustF64(blended.yScale())
                + ", xz_factor: " + rustF64(blended.xzFactor()) + ", y_factor: " + rustF64(blended.yFactor())
                + ", smear_scale_multiplier: " + rustF64(blended.smearScaleMultiplier()) + " }";
        return t;
    }

    static double firstOffsetX(Object stack) {
        Object[] layers = (Object[]) fieldAny(stack, "layers");
        return (double) fieldAny(call(layers[0], "noise"), "offsetX");
    }

    static void finalDensity(HolderLookup.Provider lookup, Path rs) throws Exception {
        NoiseGeneratorSettings settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.OVERWORLD).value();
        String structure = null;
        try (PrintWriter w = writer("final_density.txt")) {
            for (long seed : List.of(0L, INPUTS.nextLong())) {
                RandomState state = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                List<int[]> columns = new ArrayList<>(List.of(new int[] {0, 0}, new int[] {-1, 1}));
                for (int i = 0; i < 3; i++) columns.add(new int[] {INPUTS.nextInt(-100_000, 100_000), INPUTS.nextInt(-100_000, 100_000)});
                for (int[] xz : columns) {
                    Column col = column(state, settings, xz[0], xz[1]);
                    TreeExport t = exportFinalDensity(lookup, state, settings, col, seed);
                    String s = String.join("\n", t.nodes) + String.join("\n", t.noises);
                    if (structure == null) {
                        structure = s;
                        writeDensityTree(t, rs);
                    } else if (!structure.equals(s)) {
                        throw new IllegalStateException("compiled tree differs between columns or seeds");
                    }
                    // every node, in id order, each in a fresh caching context (caches carry state
                    // between calls): a 3x1x3 layer with step 4 (as maxSurfaceLevel samples), then 4
                    // points (point mode, which can read the cached layer), then the column
                    List<Object> byId = new ArrayList<>(java.util.Collections.nCopies(t.nodes.size(), null));
                    t.ids.forEach((o, id) -> byId.set(id, o));
                    StringBuilder sb = new StringBuilder(seed + " " + xz[0] + " " + xz[1]);
                    var layer = new net.minecraft.world.level.levelgen.densityfunction.DensityVolume(3, 1, 3, xz[0] - 4, 0, xz[1] - 4, 4, 1, 4);
                    int[] ys = {-64, -10, 63, 140};
                    for (Object node : byId) {
                        DensitySampler d = (DensitySampler) node;
                        var ctx = net.minecraft.world.level.levelgen.densityfunction.SamplerContext.builder().enableCaches().build();
                        sb.append(' ').append(volumeHash(d, ctx, layer)).append('|');
                        for (int i = 0; i < ys.length; i++) {
                            sb.append(i == 0 ? "" : "/").append(bits(d.sampleValue(ctx, xz[0] + i, ys[i], xz[1] - i)));
                        }
                        sb.append('|').append(volumeHash(d, ctx, col.volume()));
                    }
                    w.println(sb);
                    col.chunk().close();
                }
            }
        }
    }

    static void writeDensityTree(TreeExport t, Path rs) throws IOException {
        StringBuilder out = new StringBuilder();
        out.append("// @generated by parity/game-oracle/GameOracle.java from Minecraft ")
                .append(Files.readString(out().resolve("VERSION")).trim()).append(". Do not edit.\n");
        out.append("//! The overworld `final_density` exactly as the game compiles it for\n");
        out.append("//! `getBaseHeight` (one `NoiseChunk` column).\n\n");
        out.append("use super::tree::{Axis, DistanceMetric, Node, NoiseSource, Tree};\n");
        out.append("use crate::biome::spline::{Multipoint, Spline};\nuse crate::noise::blended::BlendedNoiseParams;\n\n");
        out.append(densityTreeSection(t, "", "OVERWORLD", "overworld", false));
        Files.writeString(rs, out);
    }

    /** The statics of one compiled tree: nodes, noises and one `Tree` per root. */
    static String densityTreeSection(TreeExport t, String prefix, String name, String file, boolean legacy) {
        StringBuilder out = new StringBuilder();
        out.append("static ").append(prefix).append("NODES: [Node; ").append(t.nodes.size()).append("] = [\n");
        for (int i = 0; i < t.nodes.size(); i++) out.append("    /* ").append(i).append(" */ ").append(t.nodes.get(i)).append(",\n");
        out.append("];\n\nstatic ").append(prefix).append("NOISES: [NoiseSource; ").append(t.noises.size()).append("] = [\n");
        for (String n : t.noises) out.append("    ").append(n).append(",\n");
        out.append("];\n");
        for (var e : t.roots.entrySet()) {
            out.append("\n/// The compiled `").append(e.getKey().toLowerCase()).append("` of `noise_settings/").append(file).append(".json`.\n");
            out.append("pub static ").append(name).append("_").append(e.getKey()).append(": Tree = Tree {\n    nodes: &").append(prefix)
                    .append("NODES,\n    root: ").append(e.getValue()).append(",\n    noises: &").append(prefix)
                    .append("NOISES,\n    blended: Some(").append(t.blendedParams).append("),\n    legacy_random: ").append(legacy).append(",\n};\n");
        }
        return out.toString();
    }

    /** FNV hash + first/middle/last values of a node over a volume, or "x" if the game rejects that volume. */
    static String volumeHash(DensitySampler d, net.minecraft.world.level.levelgen.densityfunction.SamplerContext ctx,
            net.minecraft.world.level.levelgen.densityfunction.DensityVolume volume) {
        int size = volume.size();
        var buffer = net.minecraft.world.level.levelgen.densityfunction.DensityBuffer.createUnpooled(size);
        try {
            d.sampleVolume(ctx, buffer, volume);
        } catch (IllegalArgumentException e) {
            return "x";
        }
        return Long.toHexString(fnv(buffer, size)) + "," + bits(buffer.get(0)) + "," + bits(buffer.get(size / 2)) + "," + bits(buffer.get(size - 1));
    }

    // ---- terrain: getBaseHeight / getBaseColumn through the real generator --------
    static char substance(net.minecraft.world.level.block.state.BlockState state, NoiseGeneratorSettings settings) {
        if (state.isAir()) return 'a';
        if (state.is(net.minecraft.world.level.block.Blocks.WATER)) return 'w';
        if (state.is(net.minecraft.world.level.block.Blocks.LAVA)) return 'l';
        if (state == settings.defaultBlock()) return 's';
        throw new IllegalStateException("unexpected block " + state);
    }

    static void terrain(HolderLookup.Provider lookup) throws IOException {
        var settingsHolder = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        NoiseGeneratorSettings settings = settingsHolder.value();
        var biomeSource = net.minecraft.world.level.biome.MultiNoiseBiomeSource.createFromPreset(
                lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
                        .getOrThrow(net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists.OVERWORLD));
        var generator = new net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator(biomeSource, settingsHolder);
        var height = net.minecraft.world.level.LevelHeightAccessor.create(settings.noiseSettings().minY(), settings.noiseSettings().height());
        // OCEAN_FLOOR_WG tests BlockTags.BLOCKS_MOTION_IN_HEIGHTMAP, which is not bound in a
        // world-less registry lookup; apply the same tag, resolved from the game's data, to
        // the game's own column instead.
        java.util.Set<String> motionBlocking = new java.util.HashSet<>();
        try (ZipFile jar = new ZipFile(Path.of(NormalNoise.class.getProtectionDomain().getCodeSource().getLocation().getPath()).toFile())) {
            List<String> list = new ArrayList<>();
            resolveTag(jar, "block", Identifier.parse("minecraft:blocks_motion_in_heightmap"), list);
            motionBlocking.addAll(list);
        }
        try (PrintWriter w = writer("terrain.txt")) {
            for (long seed : List.of(0L, -4172144997902289642L, INPUTS.nextLong())) {
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                List<int[]> columns = new ArrayList<>(List.of(new int[] {0, 0}, new int[] {8, -8}, new int[] {-1000, 1000}));
                for (int i = 0; i < 30; i++) {
                    int range = i % 3 == 0 ? 2_000 : i % 3 == 1 ? 50_000 : 2_000_000;
                    columns.add(new int[] {INPUTS.nextInt(-range, range), INPUTS.nextInt(-range, range)});
                }
                for (int[] xz : columns) {
                    int ws = generator.getBaseHeight(xz[0], xz[1], net.minecraft.world.level.levelgen.Heightmap.Types.WORLD_SURFACE_WG, height, rs);
                    var column = generator.getBaseColumn(xz[0], xz[1], height, rs);
                    int of = height.getMinY();
                    for (int y = height.getMinY() + height.getHeight() - 1; y >= height.getMinY(); y--) {
                        String id = net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(column.getBlock(y).getBlock()).toString();
                        if (motionBlocking.contains(id)) {
                            of = y + 1;
                            break;
                        }
                    }
                    StringBuilder blocks = new StringBuilder();
                    for (int y = height.getMinY(); y < height.getMinY() + height.getHeight(); y++) blocks.append(substance(column.getBlock(y), settings));
                    w.println(seed + " " + xz[0] + " " + xz[1] + " " + ws + " " + of + " " + blocks);
                }
            }
        }
    }

    /** NoiseBasedAquifer.computeSubstance directly, with densities near 0 where barriers and pressure decide. */
    static void aquifers(HolderLookup.Provider lookup) throws Exception {
        NoiseGeneratorSettings settings = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                .getOrThrow(NoiseGeneratorSettings.OVERWORLD).value();
        double[] densities = {-0.6, -0.2, -0.08, -0.03, -0.01, -0.002};
        try (PrintWriter w = writer("aquifer.txt")) {
            for (long seed : List.of(0L, INPUTS.nextLong())) {
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                for (int i = 0; i < 150; i++) {
                    int range = i % 2 == 0 ? 3_000 : 1_000_000;
                    int x = INPUTS.nextInt(-range, range), z = INPUTS.nextInt(-range, range);
                    Column col = column(rs, settings, x, z);
                    var aquifer = col.chunk().aquifer();
                    StringBuilder sb = new StringBuilder(seed + " " + x + " " + z + " ");
                    for (int y = 120; y >= -60; y--) {
                        for (double d : densities) {
                            var state = aquifer.computeSubstance(x, y, z, d);
                            sb.append(substance(state == null ? settings.defaultBlock() : state, settings));
                        }
                    }
                    w.println(sb);
                    col.chunk().close();
                }
            }
        }
    }

    // ---- structures: which structure (if any) generates at each candidate chunk ----

    static net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager templates;

    /** The game's structure template manager, reading templates from the server jar. */
    static net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager templateManager(HolderLookup.Provider lookup) throws IOException {
        if (templates != null) return templates;
        // The game's data (structure templates) straight from the server jar.
        var jarFs = java.nio.file.FileSystems.newFileSystem(
                Path.of(NormalNoise.class.getProtectionDomain().getCodeSource().getLocation().getPath()));
        var pack = new net.minecraft.server.packs.PathPackResources(new net.minecraft.server.packs.PackLocationInfo("vanilla",
                net.minecraft.network.chat.Component.literal("vanilla"), net.minecraft.server.packs.repository.PackSource.BUILT_IN,
                java.util.Optional.empty()), jarFs.getPath("/"));
        var resources = new net.minecraft.server.packs.resources.MultiPackResourceManager(
                net.minecraft.server.packs.PackType.SERVER_DATA, List.<net.minecraft.server.packs.PackResources>of(pack));
        // The template manager wants a world folder for saved templates; an empty one under
        // the oracle's working directory (the git-ignored cache) is enough.
        var storage = net.minecraft.world.level.storage.LevelStorageSource.createDefault(Path.of("oracle-worlds").toAbsolutePath())
                .createAccess("structures");
        templates = new net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager(
                resources, storage, net.minecraft.util.datafix.DataFixers.getDataFixer(), lookup.lookupOrThrow(Registries.BLOCK));
        if (templates.get(Identifier.parse("minecraft:village/plains/town_centers/plains_fountain_01")).isEmpty()) {
            throw new IllegalStateException("structure templates did not load");
        }
        return templates;
    }

    /** Binds every biome holder to the biome tags it is in, resolved from the game's data. */
    static void bindBiomeTags(HolderLookup.Provider lookup) throws Exception {
        Map<String, List<net.minecraft.tags.TagKey<Biome>>> byBiome = new java.util.HashMap<>();
        try (ZipFile jar = new ZipFile(Path.of(NormalNoise.class.getProtectionDomain().getCodeSource().getLocation().getPath()).toFile())) {
            String prefix = "data/minecraft/tags/worldgen/biome/";
            for (Enumeration<? extends ZipEntry> en = jar.entries(); en.hasMoreElements(); ) {
                String n = en.nextElement().getName();
                if (!n.startsWith(prefix) || !n.endsWith(".json")) continue;
                Identifier tag = Identifier.parse("minecraft:" + n.substring(prefix.length(), n.length() - 5));
                List<String> biomes = new ArrayList<>();
                resolveTag(jar, tag, biomes);
                var key = net.minecraft.tags.TagKey.create(Registries.BIOME, tag);
                for (String b : biomes) byBiome.computeIfAbsent(b, k -> new ArrayList<>()).add(key);
            }
        }
        Method bind = Holder.Reference.class.getDeclaredMethod("bindTags", java.util.Collection.class);
        bind.setAccessible(true);
        for (var h : lookup.lookupOrThrow(Registries.BIOME).listElements().toList()) {
            bind.invoke(h, byBiome.getOrDefault(h.key().identifier().toString(), List.of()));
        }
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    static net.minecraft.core.Registry<?> copyRegistry(HolderLookup.Provider lookup, ResourceKey<? extends net.minecraft.core.Registry<?>> key) {
        var registry = new net.minecraft.core.MappedRegistry(key, com.mojang.serialization.Lifecycle.stable());
        for (var ref : lookup.lookupOrThrow((ResourceKey) key).listElements().toList()) {
            var r = (Holder.Reference<?>) ref;
            registry.register(r.key(), r.value(), net.minecraft.core.RegistrationInfo.BUILT_IN);
        }
        return registry.freeze();
    }

    /** Binds the built-in block registry's tags, resolved from the game's data (pieces read some). */
    static void bindBlockTags() throws Exception {
        Map<net.minecraft.tags.TagKey<net.minecraft.world.level.block.Block>, List<Holder<net.minecraft.world.level.block.Block>>> tags = new java.util.HashMap<>();
        var blocks = net.minecraft.core.registries.BuiltInRegistries.BLOCK;
        try (ZipFile jar = new ZipFile(Path.of(NormalNoise.class.getProtectionDomain().getCodeSource().getLocation().getPath()).toFile())) {
            String prefix = "data/minecraft/tags/block/";
            for (Enumeration<? extends ZipEntry> en = jar.entries(); en.hasMoreElements(); ) {
                String n = en.nextElement().getName();
                if (!n.startsWith(prefix) || !n.endsWith(".json")) continue;
                Identifier tag = Identifier.parse("minecraft:" + n.substring(prefix.length(), n.length() - 5));
                List<String> ids = new ArrayList<>();
                resolveTag(jar, "block", tag, ids);
                List<Holder<net.minecraft.world.level.block.Block>> holders = new ArrayList<>();
                for (String id : ids) blocks.get(Identifier.parse(id)).ifPresent(holders::add);
                tags.put(net.minecraft.tags.TagKey.create(Registries.BLOCK, tag), holders);
            }
        }
        // How the server binds data-pack tags to a frozen registry.
        blocks.prepareTagReload(new net.minecraft.tags.TagLoader.LoadResult<>(Registries.BLOCK, tags)).apply();
    }

    static void structures(HolderLookup.Provider lookup) throws Exception {
        bindBiomeTags(lookup);
        bindBlockTags();
        var settingsHolder = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        NoiseGeneratorSettings settings = settingsHolder.value();
        var biomeSource = net.minecraft.world.level.biome.MultiNoiseBiomeSource.createFromPreset(
                lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
                        .getOrThrow(net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists.OVERWORLD));
        var generator = new net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator(biomeSource, settingsHolder);
        var height = net.minecraft.world.level.LevelHeightAccessor.create(settings.noiseSettings().minY(), settings.noiseSettings().height());
        // Structure pieces read template pools, processor lists... through a RegistryAccess:
        // copy the ones they use from the world-less lookup into frozen registries.
        List<net.minecraft.core.Registry<?>> registries = new ArrayList<>();
        for (var key : List.of(Registries.TEMPLATE_POOL, Registries.PROCESSOR_LIST)) registries.add(copyRegistry(lookup, key));
        net.minecraft.core.registries.BuiltInRegistries.REGISTRY.forEach(registries::add);
        var registryAccess = new net.minecraft.core.RegistryAccess.ImmutableRegistryAccess(registries);
        var templates = templateManager(lookup);
        // Structure biome tags are not bound to the structures in a world-less lookup:
        // test membership against the same tags, resolved from the game's data.
        Map<String, java.util.Set<String>> structureBiomes = new java.util.HashMap<>();
        for (var ref : lookup.lookupOrThrow(Registries.STRUCTURE).listElements().toList()) {
            String tag = ((net.minecraft.core.HolderSet.Named<Biome>) ref.value().biomes()).key().location().toString();
            structureBiomes.put(ref.key().identifier().toString(), new java.util.HashSet<>(biomeTag(tag)));
        }
        java.util.Set<String> skip = java.util.Set.of("minecraft:strongholds", "minecraft:nether_complexes",
                "minecraft:nether_fossils", "minecraft:end_cities");
        var sets = structureSets(lookup);
        List<Long> worldSeeds = List.of(0L, -4172144997902289642L, INPUTS.nextLong());
        try (PrintWriter w = writer("structures.txt")) {
            for (long seed : worldSeeds) {
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                var ctor = net.minecraft.world.level.chunk.ChunkGeneratorStructureState.class.getDeclaredConstructor(
                        RandomState.class, net.minecraft.world.level.biome.BiomeSource.class, long.class,
                        net.minecraft.world.level.ChunkPos.class, long.class, List.class);
                ctor.setAccessible(true);
                var state = ctor.newInstance(rs, biomeSource, seed, new net.minecraft.world.level.ChunkPos(0, 0), seed, List.copyOf(sets));
                for (var setRef : sets) {
                    String setName = setRef.key().identifier().toString();
                    if (skip.contains(setName)) continue;
                    var set = setRef.value();
                    var placement = (net.minecraft.world.level.levelgen.structure.placement.RandomSpreadStructurePlacement) set.placement();
                    List<int[]> chunks = new ArrayList<>();
                    if (placement.spacing() > 1) {
                        for (int rz = -6; rz < 6; rz++) for (int rx = -6; rx < 6; rx++) {
                            var c = placement.getPotentialStructureChunk(seed, rx * placement.spacing(), rz * placement.spacing());
                            chunks.add(new int[] {c.x(), c.z()});
                        }
                        for (int i = 0; i < 30; i++) {
                            int range = 2_000_000 / 16 / placement.spacing();
                            int rx = INPUTS.nextInt(-range, range), rz = INPUTS.nextInt(-range, range);
                            var c = placement.getPotentialStructureChunk(seed, rx * placement.spacing(), rz * placement.spacing());
                            chunks.add(new int[] {c.x(), c.z()});
                        }
                    } else {
                        for (int cz = -48; cz < 48; cz++) for (int cx = -48; cx < 48; cx++) chunks.add(new int[] {cx, cz});
                    }
                    for (int[] c : chunks) {
                        var pos = new net.minecraft.world.level.ChunkPos(c[0], c[1]);
                        if (!placement.isStructureChunk(state, pos.x(), pos.z())) continue;
                        StringBuilder sb = new StringBuilder(seed + " " + setName + " " + pos.x() + " " + pos.z());
                        selectStructure(sb, set, seed, pos, registryAccess, generator, biomeSource, rs, templates, height, structureBiomes);
                        w.println(sb);
                    }
                }
            }
        }
        // Each structure's generation point on clusters of chunks, wherever the sets would
        // attempt them or not: many more valid points and edge cases per structure type.
        List<String> names = new ArrayList<>();
        for (var setRef : sets) {
            if (skip.contains(setRef.key().identifier().toString())) continue;
            for (var e : setRef.value().structures()) names.add(e.structure().unwrapKey().orElseThrow().identifier().toString());
        }
        java.util.Collections.sort(names);
        try (PrintWriter w = writer("structure_points.txt")) {
            long seed = INPUTS.nextLong();
            RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
            var registry = lookup.lookupOrThrow(Registries.STRUCTURE);
            for (String name : names) {
                if (name.contains("mineshaft") || name.contains("ruined_portal")) continue;
                var holder = registry.getOrThrow(ResourceKey.create(Registries.STRUCTURE, Identifier.parse(name)));
                var biomes = structureBiomes.get(name);
                java.util.function.Predicate<Holder<Biome>> valid = h -> biomes.contains(h.unwrapKey().orElseThrow().identifier().toString());
                StringBuilder sb = new StringBuilder(seed + " " + name.substring("minecraft:".length()));
                var finder = biomeSource.createResolver(rs.createClimateSampler(SamplerContext.builder().build()));
                for (int cluster = 0; cluster < 12; cluster++) {
                    // Center each cluster on one of the structure's biomes (at the surface), if found.
                    int ccx = 0, ccz = 0;
                    for (int tries = 0; tries < 4000; tries++) {
                        ccx = INPUTS.nextInt(-6000, 6000);
                        ccz = INPUTS.nextInt(-6000, 6000);
                        var b = finder.getNoiseBiome(ccx * 4 + 2, 16, ccz * 4 + 2);
                        if (valid.test(b)) break;
                    }
                    ccx -= 3;
                    ccz -= 3;
                    for (int dz = 0; dz < 6; dz++) for (int dx = 0; dx < 6; dx++) {
                        var pos = new net.minecraft.world.level.ChunkPos(ccx + dx, ccz + dz);
                        var context = new net.minecraft.world.level.levelgen.structure.Structure.GenerationContext(
                                registryAccess, generator, biomeSource, rs.createClimateSampler(SamplerContext.builder().enableCaches().build()),
                                rs, templates, seed, pos, height, valid);
                        var point = holder.value().findValidGenerationPoint(context);
                        sb.append(' ').append(pos.x()).append(',').append(pos.z()).append(':');
                        if (point.isPresent()) {
                            var p = point.get().position();
                            sb.append(p.getX()).append(',').append(p.getY()).append(',').append(p.getZ());
                        } else {
                            sb.append("no");
                        }
                    }
                }
                w.println(sb);
            }
            // Woodland mansions need their lowest corner at y >= 60: find chunks right at that edge.
            var mansion = registry.getOrThrow(ResourceKey.create(Registries.STRUCTURE, Identifier.parse("minecraft:mansion")));
            var mansionBiomes = structureBiomes.get("minecraft:mansion");
            java.util.function.Predicate<Holder<Biome>> mansionValid = h -> mansionBiomes.contains(h.unwrapKey().orElseThrow().identifier().toString());
            Method lowest = net.minecraft.world.level.levelgen.structure.Structure.class.getDeclaredMethod("getLowestYIn5by5Box",
                    net.minecraft.world.level.levelgen.structure.Structure.GenerationContext.class, int.class, int.class,
                    net.minecraft.world.level.block.Rotation.class);
            lowest.setAccessible(true);
            var finder = biomeSource.createResolver(rs.createClimateSampler(SamplerContext.builder().build()));
            StringBuilder sb = new StringBuilder(seed + " mansion");
            int found = 0;
            for (int i = 0; i < 150_000 && found < 40; i++) {
                int cx = INPUTS.nextInt(-20000, 20000), cz = INPUTS.nextInt(-20000, 20000);
                if (!mansionValid.test(finder.getNoiseBiome(cx * 4 + 1, 16, cz * 4 + 1))) continue;
                var pos = new net.minecraft.world.level.ChunkPos(cx, cz);
                var context = new net.minecraft.world.level.levelgen.structure.Structure.GenerationContext(
                        registryAccess, generator, biomeSource, rs.createClimateSampler(SamplerContext.builder().enableCaches().build()),
                        rs, templates, seed, pos, height, mansionValid);
                var rotation = net.minecraft.world.level.block.Rotation.getRandom(context.random());
                int y = ((net.minecraft.core.BlockPos) lowest.invoke(mansion.value(), context, pos.getBlockX(7), pos.getBlockZ(7), rotation)).getY();
                if (y < 58 || y > 61) continue;
                found++;
                var check = new net.minecraft.world.level.levelgen.structure.Structure.GenerationContext(
                        registryAccess, generator, biomeSource, rs.createClimateSampler(SamplerContext.builder().enableCaches().build()),
                        rs, templates, seed, pos, height, mansionValid);
                var point = mansion.value().findValidGenerationPoint(check);
                sb.append(' ').append(cx).append(',').append(cz).append(':');
                if (point.isPresent()) {
                    var p = point.get().position();
                    sb.append(p.getX()).append(',').append(p.getY()).append(',').append(p.getZ());
                } else {
                    sb.append("no");
                }
            }
            w.println(sb);
        }
    }

    /** The Nether's structure attempts (nether_complexes, nether_fossils) and generation points. */
    static void netherStructures(HolderLookup.Provider lookup) throws Exception {
        var settingsHolder = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.NETHER);
        NoiseGeneratorSettings settings = settingsHolder.value();
        var biomeSource = net.minecraft.world.level.biome.MultiNoiseBiomeSource.createFromPreset(
                lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
                        .getOrThrow(net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists.NETHER));
        var generator = new net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator(biomeSource, settingsHolder);
        // The level's height is the dimension type's (dimension_type/the_nether.json: y 0 to 255),
        // not the noise settings' 128 blocks.
        var height = net.minecraft.world.level.LevelHeightAccessor.create(0, 256);
        List<net.minecraft.core.Registry<?>> registries = new ArrayList<>();
        for (var key : List.of(Registries.TEMPLATE_POOL, Registries.PROCESSOR_LIST)) registries.add(copyRegistry(lookup, key));
        net.minecraft.core.registries.BuiltInRegistries.REGISTRY.forEach(registries::add);
        var registryAccess = new net.minecraft.core.RegistryAccess.ImmutableRegistryAccess(registries);
        var templates = templateManager(lookup);
        Map<String, java.util.Set<String>> structureBiomes = new java.util.HashMap<>();
        for (var ref : lookup.lookupOrThrow(Registries.STRUCTURE).listElements().toList()) {
            String tag = ((net.minecraft.core.HolderSet.Named<Biome>) ref.value().biomes()).key().location().toString();
            structureBiomes.put(ref.key().identifier().toString(), new java.util.HashSet<>(biomeTag(tag)));
        }
        var sets = structureSets(lookup);
        List<String> netherSets = List.of("minecraft:nether_complexes", "minecraft:nether_fossils");
        List<Long> worldSeeds = List.of(0L, -4172144997902289642L, INPUTS.nextLong());
        try (PrintWriter w = writer("nether_structures.txt")) {
            for (long seed : worldSeeds) {
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                var ctor = net.minecraft.world.level.chunk.ChunkGeneratorStructureState.class.getDeclaredConstructor(
                        RandomState.class, net.minecraft.world.level.biome.BiomeSource.class, long.class,
                        net.minecraft.world.level.ChunkPos.class, long.class, List.class);
                ctor.setAccessible(true);
                var state = ctor.newInstance(rs, biomeSource, seed, new net.minecraft.world.level.ChunkPos(0, 0), seed, List.copyOf(sets));
                for (var setRef : sets) {
                    String setName = setRef.key().identifier().toString();
                    if (!netherSets.contains(setName)) continue;
                    var set = setRef.value();
                    var placement = (net.minecraft.world.level.levelgen.structure.placement.RandomSpreadStructurePlacement) set.placement();
                    List<int[]> chunks = new ArrayList<>();
                    for (int rz = -8; rz < 8; rz++) for (int rx = -8; rx < 8; rx++) {
                        var c = placement.getPotentialStructureChunk(seed, rx * placement.spacing(), rz * placement.spacing());
                        chunks.add(new int[] {c.x(), c.z()});
                    }
                    for (int i = 0; i < 40; i++) {
                        int range = 2_000_000 / 16 / placement.spacing();
                        int rx = INPUTS.nextInt(-range, range), rz = INPUTS.nextInt(-range, range);
                        var c = placement.getPotentialStructureChunk(seed, rx * placement.spacing(), rz * placement.spacing());
                        chunks.add(new int[] {c.x(), c.z()});
                    }
                    for (int[] c : chunks) {
                        var pos = new net.minecraft.world.level.ChunkPos(c[0], c[1]);
                        if (!placement.isStructureChunk(state, pos.x(), pos.z())) continue;
                        StringBuilder sb = new StringBuilder(seed + " " + setName + " " + pos.x() + " " + pos.z());
                        selectStructure(sb, set, seed, pos, registryAccess, generator, biomeSource, rs, templates, height, structureBiomes);
                        w.println(sb);
                    }
                }
            }
        }
        // Generation points of each Nether structure on random clusters of chunks.
        try (PrintWriter w = writer("nether_structure_points.txt")) {
            long seed = INPUTS.nextLong();
            RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
            var registry = lookup.lookupOrThrow(Registries.STRUCTURE);
            for (String name : List.of("minecraft:bastion_remnant", "minecraft:fortress", "minecraft:nether_fossil")) {
                var holder = registry.getOrThrow(ResourceKey.create(Registries.STRUCTURE, Identifier.parse(name)));
                var biomes = structureBiomes.get(name);
                java.util.function.Predicate<Holder<Biome>> valid = h -> biomes.contains(h.unwrapKey().orElseThrow().identifier().toString());
                StringBuilder sb = new StringBuilder(seed + " " + name.substring("minecraft:".length()));
                for (int cluster = 0; cluster < 12; cluster++) {
                    int ccx = INPUTS.nextInt(-6000, 6000), ccz = INPUTS.nextInt(-6000, 6000);
                    for (int dz = 0; dz < 6; dz++) for (int dx = 0; dx < 6; dx++) {
                        var pos = new net.minecraft.world.level.ChunkPos(ccx + dx, ccz + dz);
                        var context = new net.minecraft.world.level.levelgen.structure.Structure.GenerationContext(
                                registryAccess, generator, biomeSource, rs.createClimateSampler(SamplerContext.builder().enableCaches().build()),
                                rs, templates, seed, pos, height, valid);
                        var point = holder.value().findValidGenerationPoint(context);
                        sb.append(' ').append(pos.x()).append(',').append(pos.z()).append(':');
                        if (point.isPresent()) {
                            var p = point.get().position();
                            sb.append(p.getX()).append(',').append(p.getY()).append(',').append(p.getZ());
                        } else {
                            sb.append("no");
                        }
                    }
                }
                w.println(sb);
            }
        }
    }

    /**
     * Ruined portals: the Nether's ruined_portals attempts (the overworld's are in structures.txt),
     * and the generation point of every variant on clusters of chunks, in both dimensions.
     */
    static void ruinedPortals(HolderLookup.Provider lookup) throws Exception {
        List<net.minecraft.core.Registry<?>> registries = new ArrayList<>();
        for (var key : List.of(Registries.TEMPLATE_POOL, Registries.PROCESSOR_LIST)) registries.add(copyRegistry(lookup, key));
        net.minecraft.core.registries.BuiltInRegistries.REGISTRY.forEach(registries::add);
        var registryAccess = new net.minecraft.core.RegistryAccess.ImmutableRegistryAccess(registries);
        var templates = templateManager(lookup);
        Map<String, java.util.Set<String>> structureBiomes = new java.util.HashMap<>();
        for (var ref : lookup.lookupOrThrow(Registries.STRUCTURE).listElements().toList()) {
            String tag = ((net.minecraft.core.HolderSet.Named<Biome>) ref.value().biomes()).key().location().toString();
            structureBiomes.put(ref.key().identifier().toString(), new java.util.HashSet<>(biomeTag(tag)));
        }
        var sets = structureSets(lookup);
        var registry = lookup.lookupOrThrow(Registries.STRUCTURE);
        var portalSet = sets.stream().filter(r -> r.key().identifier().toString().equals("minecraft:ruined_portals")).findFirst().orElseThrow();
        List<String> names = new ArrayList<>();
        for (var e : portalSet.value().structures()) names.add(e.structure().unwrapKey().orElseThrow().identifier().toString());
        java.util.Collections.sort(names);
        try (PrintWriter attempts = writer("ruined_portals.txt"); PrintWriter points = writer("ruined_portal_points.txt")) {
            for (boolean nether : new boolean[] {false, true}) {
                var settingsHolder = lookup.lookupOrThrow(Registries.NOISE_SETTINGS)
                        .getOrThrow(nether ? NoiseGeneratorSettings.NETHER : NoiseGeneratorSettings.OVERWORLD);
                NoiseGeneratorSettings settings = settingsHolder.value();
                var biomeSource = net.minecraft.world.level.biome.MultiNoiseBiomeSource.createFromPreset(
                        lookup.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
                                .getOrThrow(nether ? net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists.NETHER
                                        : net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists.OVERWORLD));
                var generator = new net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator(biomeSource, settingsHolder);
                var height = nether ? net.minecraft.world.level.LevelHeightAccessor.create(0, 256)
                        : net.minecraft.world.level.LevelHeightAccessor.create(settings.noiseSettings().minY(), settings.noiseSettings().height());
                String dim = nether ? "nether" : "overworld";
                if (nether) {
                    var placement = (net.minecraft.world.level.levelgen.structure.placement.RandomSpreadStructurePlacement) portalSet.value().placement();
                    for (long seed : List.of(0L, -4172144997902289642L, INPUTS.nextLong())) {
                        RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                        var ctor = net.minecraft.world.level.chunk.ChunkGeneratorStructureState.class.getDeclaredConstructor(
                                RandomState.class, net.minecraft.world.level.biome.BiomeSource.class, long.class,
                                net.minecraft.world.level.ChunkPos.class, long.class, List.class);
                        ctor.setAccessible(true);
                        var state = ctor.newInstance(rs, biomeSource, seed, new net.minecraft.world.level.ChunkPos(0, 0), seed, List.copyOf(sets));
                        List<int[]> chunks = new ArrayList<>();
                        for (int rz = -8; rz < 8; rz++) for (int rx = -8; rx < 8; rx++) chunks.add(new int[] {rx, rz});
                        for (int i = 0; i < 60; i++) {
                            int range = 2_000_000 / 16 / placement.spacing();
                            chunks.add(new int[] {INPUTS.nextInt(-range, range), INPUTS.nextInt(-range, range)});
                        }
                        for (int[] r : chunks) {
                            var pos = placement.getPotentialStructureChunk(seed, r[0] * placement.spacing(), r[1] * placement.spacing());
                            if (!placement.isStructureChunk(state, pos.x(), pos.z())) continue;
                            StringBuilder sb = new StringBuilder(seed + " minecraft:ruined_portals " + pos.x() + " " + pos.z());
                            selectStructure(sb, portalSet.value(), seed, pos, registryAccess, generator, biomeSource, rs, templates, height, structureBiomes);
                            attempts.println(sb);
                        }
                    }
                }
                long seed = INPUTS.nextLong();
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                var finder = biomeSource.createResolver(rs.createClimateSampler(SamplerContext.builder().build()));
                for (String name : names) {
                    if (nether != name.equals("minecraft:ruined_portal_nether")) continue;
                    var holder = registry.getOrThrow(ResourceKey.create(Registries.STRUCTURE, Identifier.parse(name)));
                    var biomes = structureBiomes.get(name);
                    java.util.function.Predicate<Holder<Biome>> valid = h -> biomes.contains(h.unwrapKey().orElseThrow().identifier().toString());
                    StringBuilder sb = new StringBuilder(dim + " " + seed + " " + name.substring("minecraft:".length()));
                    for (int cluster = 0; cluster < (nether ? 40 : 16); cluster++) {
                        int ccx = 0, ccz = 0;
                        for (int tries = 0; tries < 4000; tries++) {
                            ccx = INPUTS.nextInt(-6000, 6000);
                            ccz = INPUTS.nextInt(-6000, 6000);
                            if (valid.test(finder.getNoiseBiome(ccx * 4 + 2, 16, ccz * 4 + 2))) break;
                        }
                        ccx -= 3;
                        ccz -= 3;
                        for (int dz = 0; dz < 6; dz++) for (int dx = 0; dx < 6; dx++) {
                            var pos = new net.minecraft.world.level.ChunkPos(ccx + dx, ccz + dz);
                            var context = new net.minecraft.world.level.levelgen.structure.Structure.GenerationContext(
                                    registryAccess, generator, biomeSource, rs.createClimateSampler(SamplerContext.builder().enableCaches().build()),
                                    rs, templates, seed, pos, height, h -> true);
                            // Without the biome check: every point findSuitableY gives, valid biome or not.
                            var stub = holder.value().findValidGenerationPoint(context);
                            sb.append(' ').append(pos.x()).append(',').append(pos.z()).append(':');
                            if (stub.isPresent()) {
                                var p = stub.get().position();
                                sb.append(p.getX()).append(',').append(p.getY()).append(',').append(p.getZ());
                            } else {
                                sb.append("no");
                            }
                        }
                    }
                    points.println(sb);
                }
            }
        }
    }

    /**
     * SimplexNoise (2D) and EndIslandFunction.getHeightValue, seeded like EndIslandFunction.compileSampler:
     * LegacyRandomSource(seed), 17292 values skipped, then new SimplexNoise(random, true).
     */
    static void endIslands() throws Exception {
        Method height = net.minecraft.world.level.levelgen.densityfunction.generator.EndIslandFunction.class
                .getDeclaredMethod("getHeightValue", net.minecraft.world.level.levelgen.synth.SimplexNoise.class, int.class, int.class);
        height.setAccessible(true);
        try (PrintWriter w = writer("end_islands.txt")) {
            // Seeds with an island cell right on the main island's edge (x^2 + z^2 == 4096).
            List<Long> edgeSeeds = new ArrayList<>();
            for (long seed = 0; edgeSeeds.size() < 3; seed++) {
                RandomSource r = new LegacyRandomSource(seed);
                r.consumeCount(17292);
                var noise = new net.minecraft.world.level.levelgen.synth.SimplexNoise(r, true);
                if (noise.get(64, 0) < -0.9f || noise.get(-64, 0) < -0.9f || noise.get(0, 64) < -0.9f || noise.get(0, -64) < -0.9f) edgeSeeds.add(seed);
            }
            for (long seed : edgeSeeds) {
                RandomSource r = new LegacyRandomSource(seed);
                r.consumeCount(17292);
                var noise = new net.minecraft.world.level.levelgen.synth.SimplexNoise(r, true);
                StringBuilder sb = new StringBuilder("height " + seed);
                for (int x = -150; x <= 150; x += 3) for (int z = -150; z <= 150; z += 3) {
                    if (Math.abs(x) < 110 && Math.abs(z) < 110) continue;
                    sb.append(' ').append(x).append(',').append(z).append('=')
                            .append(Float.floatToRawIntBits((float) height.invoke(null, noise, x, z)));
                }
                w.println(sb);
            }
            for (long seed : seeds(6)) {
                for (boolean zero : new boolean[] {true, false}) {
                    RandomSource r = new LegacyRandomSource(seed);
                    r.consumeCount(17292);
                    var noise = new net.minecraft.world.level.levelgen.synth.SimplexNoise(r, zero);
                    StringBuilder sb = new StringBuilder("simplex " + seed + " " + zero);
                    for (int i = 0; i < 1500; i++) {
                        // Doubles, integers (the islands sample integer cells), and the diagonal x == y.
                        double x, y;
                        if (i < 300) { x = INPUTS.nextDouble(-50, 50); y = INPUTS.nextDouble(-50, 50); }
                        else if (i < 1100) { x = INPUTS.nextInt(-5000, 5000); y = INPUTS.nextInt(-5000, 5000); }
                        else if (i < 1300) { x = INPUTS.nextInt(-5000, 5000); y = x; }
                        else { x = INPUTS.nextDouble(-3e7, 3e7); y = INPUTS.nextDouble(-3e7, 3e7); }
                        sb.append(' ').append(Double.doubleToRawLongBits(x)).append(',').append(Double.doubleToRawLongBits(y))
                                .append('=').append(Float.floatToRawIntBits(noise.get(x, y)));
                    }
                    w.println(sb);
                    if (!zero) continue;
                    sb = new StringBuilder("height " + seed);
                    for (int i = 0; i < 300; i++) {
                        // Grid coordinates (8 blocks): around the main island's edge, and far out.
                        int x, z;
                        if (i < 100) { x = INPUTS.nextInt(-160, 160); z = INPUTS.nextInt(-160, 160); }
                        else if (i < 280) { x = INPUTS.nextInt(-20000, 20000); z = INPUTS.nextInt(-20000, 20000); }
                        else { x = INPUTS.nextInt(-3_750_000, 3_750_000); z = INPUTS.nextInt(-3_750_000, 3_750_000); }
                        sb.append(' ').append(x).append(',').append(z).append('=')
                                .append(Float.floatToRawIntBits((float) height.invoke(null, noise, x, z)));
                    }
                    w.println(sb);
                }
            }
        }
    }

    /**
     * The End: its compiled final_density and the climate sampler's erosion (the islands, which
     * TheEndBiomeSource reads), base columns, and biomes from TheEndBiomeSource itself.
     */
    static void endTerrain(HolderLookup.Provider lookup, Path densityRs) throws Exception {
        var settingsHolder = lookup.lookupOrThrow(Registries.NOISE_SETTINGS).getOrThrow(NoiseGeneratorSettings.END);
        NoiseGeneratorSettings settings = settingsHolder.value();
        var biomeSource = net.minecraft.world.level.biome.TheEndBiomeSource.create(lookup.lookupOrThrow(Registries.BIOME));
        var generator = new net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator(biomeSource, settingsHolder);
        var height = net.minecraft.world.level.LevelHeightAccessor.create(settings.noiseSettings().minY(), settings.noiseSettings().height());
        String structure = null;
        List<Long> worldSeeds = new ArrayList<>(List.of(0L, 1L, -1L, -4172144997902289642L));
        for (int i = 0; i < 4; i++) worldSeeds.add(INPUTS.nextLong());
        try (PrintWriter w = writer("end_terrain.txt"); PrintWriter b = writer("end_biomes.txt")) {
            for (long seed : worldSeeds) {
                RandomState rs = RandomState.create(lookup.lookupOrThrow(Registries.NOISE), seed, settings);
                // Columns: the main island, its edge, and the outer islands.
                List<int[]> columns = new ArrayList<>(List.of(new int[] {0, 0}, new int[] {8, -8}, new int[] {90, 30}, new int[] {-120, 0}));
                for (int i = 0; i < 30; i++) {
                    int range = i % 3 == 0 ? 200 : i % 3 == 1 ? 3_000 : 1_000_000;
                    columns.add(new int[] {INPUTS.nextInt(-range, range), INPUTS.nextInt(-range, range)});
                }
                for (int[] xz : columns) {
                    Column col = column(rs, settings, xz[0], xz[1]);
                    TreeExport t = exportFinalDensity(lookup, rs, settings, col, seed);
                    t.roots.put("EROSION", t.node(rs.createClimateSampler(SamplerContext.builder().build()).erosion().sampler()));
                    String s = String.join("\n", t.nodes) + String.join("\n", t.noises);
                    if (structure == null) {
                        structure = s;
                        String file = Files.readString(densityRs);
                        Files.writeString(densityRs, file + "\n// ---- The End ----------------------------------------------------------\n\n"
                                + densityTreeSection(t, "END_", "END", "end", true));
                    } else if (!structure.equals(s)) {
                        throw new IllegalStateException("compiled End tree differs between columns or seeds");
                    }
                    col.chunk().close();
                    int ws = generator.getBaseHeight(xz[0], xz[1], net.minecraft.world.level.levelgen.Heightmap.Types.WORLD_SURFACE_WG, height, rs);
                    var column = generator.getBaseColumn(xz[0], xz[1], height, rs);
                    StringBuilder blocks = new StringBuilder();
                    for (int y = height.getMinY(); y < height.getMinY() + height.getHeight(); y++) blocks.append(substance(column.getBlock(y), settings));
                    w.println(seed + " " + xz[0] + " " + xz[1] + " " + ws + " " + blocks);
                }
                // Biomes at quart positions: near the main island's edge (64 chunks), and far out.
                var resolver = biomeSource.createResolver(rs.createClimateSampler(SamplerContext.builder().build()));
                StringBuilder sb = new StringBuilder(Long.toString(seed));
                for (int i = 0; i < 1500; i++) {
                    int range = i % 3 == 0 ? 300 : i % 3 == 1 ? 3_000 : 500_000;
                    int qx = INPUTS.nextInt(-range, range), qz = INPUTS.nextInt(-range, range), qy = INPUTS.nextInt(-10, 80);
                    sb.append(' ').append(qx).append(',').append(qy).append(',').append(qz).append('=')
                            .append(resolver.getNoiseBiome(qx, qy, qz).unwrapKey().orElseThrow().identifier().getPath());
                }
                // The main island's edge: sections with x^2 + z^2 == 4096, and their neighbours.
                for (int[] sec : new int[][] {{64, 0}, {-64, 0}, {0, 64}, {0, -64}, {65, 0}, {-65, 0}, {0, 65}, {0, -65}}) {
                    for (int dq = 0; dq < 4; dq++) {
                        int qx = sec[0] * 4 + dq, qz = sec[1] * 4 + (3 - dq), qy = 16;
                        sb.append(' ').append(qx).append(',').append(qy).append(',').append(qz).append('=')
                                .append(resolver.getNoiseBiome(qx, qy, qz).unwrapKey().orElseThrow().identifier().getPath());
                    }
                }
                b.println(sb);
            }
        }
    }

    /** ChunkGenerator.createStructures for one set and chunk (its selection loop), tracing each try. */
    static void selectStructure(StringBuilder sb, net.minecraft.world.level.levelgen.structure.StructureSet set, long seed,
            net.minecraft.world.level.ChunkPos pos, net.minecraft.core.RegistryAccess registryAccess,
            net.minecraft.world.level.chunk.ChunkGenerator generator, net.minecraft.world.level.biome.BiomeSource biomeSource,
            RandomState rs, net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager templates,
            net.minecraft.world.level.LevelHeightAccessor height, Map<String, java.util.Set<String>> structureBiomes) {
        // One climate sampler per chunk, with caches, as createStructures makes it.
        var sampler = rs.createClimateSampler(SamplerContext.builder().enableCaches().build());
        List<net.minecraft.world.level.levelgen.structure.StructureSet.StructureSelectionEntry> entries = new ArrayList<>(set.structures());
        if (entries.size() == 1) {
            tryStructure(sb, entries.get(0), seed, pos, registryAccess, generator, biomeSource, sampler, rs, templates, height, structureBiomes);
            return;
        }
        WorldgenRandom random = new WorldgenRandom(new LegacyRandomSource(0L));
        random.setLargeFeatureSeed(seed, pos.x(), pos.z());
        int total = 0;
        for (var e : entries) total += e.weight();
        while (!entries.isEmpty()) {
            int pick = random.nextInt(total);
            int i = 0;
            for (var e : entries) {
                pick -= e.weight();
                if (pick < 0) break;
                i++;
            }
            var entry = entries.get(i);
            if (tryStructure(sb, entry, seed, pos, registryAccess, generator, biomeSource, sampler, rs, templates, height, structureBiomes)) return;
            entries.remove(i);
            total -= entry.weight();
        }
    }

    static boolean tryStructure(StringBuilder sb, net.minecraft.world.level.levelgen.structure.StructureSet.StructureSelectionEntry entry,
            long seed, net.minecraft.world.level.ChunkPos pos, net.minecraft.core.RegistryAccess registryAccess,
            net.minecraft.world.level.chunk.ChunkGenerator generator, net.minecraft.world.level.biome.BiomeSource biomeSource,
            net.minecraft.world.level.biome.Climate.Sampler sampler, RandomState rs,
            net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager templates,
            net.minecraft.world.level.LevelHeightAccessor height, Map<String, java.util.Set<String>> structureBiomes) {
        String name = entry.structure().unwrapKey().orElseThrow().identifier().toString();
        var biomes = structureBiomes.get(name);
        java.util.function.Predicate<Holder<Biome>> valid = h -> biomes.contains(h.unwrapKey().orElseThrow().identifier().toString());
        var structure = entry.structure().value();
        var start = structure.generate(entry.structure(), net.minecraft.world.level.Level.OVERWORLD, registryAccess, generator,
                biomeSource, sampler, rs, templates, seed, pos, 0, height, valid);
        // The generation point the game validated, recomputed for the fixture with its own
        // climate sampler, so the shared sampler's caches see exactly the game's queries.
        var context = new net.minecraft.world.level.levelgen.structure.Structure.GenerationContext(
                registryAccess, generator, biomeSource, rs.createClimateSampler(SamplerContext.builder().enableCaches().build()),
                rs, templates, seed, pos, height, valid);
        var point = structure.findValidGenerationPoint(context);
        sb.append(' ').append(name.substring("minecraft:".length())).append(':');
        if (start.isValid()) {
            var p = point.orElseThrow().position();
            sb.append(p.getX()).append(',').append(p.getY()).append(',').append(p.getZ());
            return true;
        }
        sb.append(point.isPresent() ? "nopieces" : "no");
        return false;
    }
}
