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

import net.minecraft.util.Mth;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.PositionalRandomFactory;
import net.minecraft.world.level.levelgen.RandomSupport;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

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
}
