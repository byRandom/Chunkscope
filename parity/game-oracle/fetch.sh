# Sourced by run.sh and wide.sh: downloads the official server jar for
# $VERSION from Mojang (verified by SHA-1) into $CACHE (once), unpacks the
# game and its libraries, and sets CP to their classpath.
# Needs VERSION and CACHE.

: "${VERSION:?}" "${CACHE:?}"
mkdir -p "$CACHE"
if [[ ! -f "$CACHE/server.jar" ]]; then
  MANIFEST=https://piston-meta.mojang.com/mc/game/version_manifest_v2.json
  version_url=$(curl -fsSL "$MANIFEST" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(next(v['url'] for v in d['versions'] if v['id'] == sys.argv[1]))" "$VERSION")
  read -r url sha1 < <(curl -fsSL "$version_url" | python3 -c "
import json, sys
s = json.load(sys.stdin)['downloads']['server']
print(s['url'], s['sha1'])")
  curl -fsSL -o "$CACHE/server.jar.part" "$url"
  echo "$sha1  $CACHE/server.jar.part" | sha1sum -c --quiet
  mv "$CACHE/server.jar.part" "$CACHE/server.jar"
fi

# The server jar is a bundler: the game and its libraries are nested jars.
rm -rf "$CACHE/bundle"
unzip -qo "$CACHE/server.jar" 'META-INF/versions/*' 'META-INF/libraries/*' -d "$CACHE/bundle"
CP=$(find "$CACHE/bundle/META-INF" -name '*.jar' | sort | paste -sd: -)
