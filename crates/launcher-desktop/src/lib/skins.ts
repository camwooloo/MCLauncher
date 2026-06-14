// Built-in skin gallery. These are Mojang's official default player skins,
// served from the stable Minecraft texture CDN. Applying uses the URL directly
// (the backend downloads + uploads the bytes), and the same URL previews the
// face in the gallery. Users can also paste any skin URL or upload a file, so
// the effective catalogue is unlimited.

export interface GallerySkin {
  id: string;
  name: string;
  /** Direct PNG texture URL (64×64). */
  url: string;
  variant: "classic" | "slim";
}

const tex = (hash: string) => `https://textures.minecraft.net/texture/${hash}`;

export const SKIN_GALLERY: GallerySkin[] = [
  { id: "steve", name: "Steve", variant: "classic", url: tex("31f477eb1a7beee631c2ca64d06f8f68fa93a3386d04452ab27f43acdf1b60cb") },
  { id: "alex", name: "Alex", variant: "slim", url: tex("1abc803022d8300ab7578b189294cce39622d9a404cdc00d3feacfdf45be6981") },
  { id: "ari", name: "Ari", variant: "classic", url: tex("6ac6ca262d67bcfb3dbc924ba8215a18195497c780058a5749de674217721892") },
  { id: "efe", name: "Efe", variant: "slim", url: tex("daf3d88ccb38f11f74814e92053d92f7728ddb1a7955652a60e30cb27ae6659f") },
  { id: "kai", name: "Kai", variant: "classic", url: tex("e5cdc3243b2153ab28a159861be643a4fc1e3c17d291cdd3e57a7f370ad676f3") },
  { id: "makena", name: "Makena", variant: "slim", url: tex("dc0fcfaf2aa040a83dc0de4e56058d1bbb2ea40157501f3e7d15dc245e493095") },
  { id: "noor", name: "Noor", variant: "slim", url: tex("90e75cd429ba6331cd210b9bd19399527ee3bab467b5a9f61cb8a27b177f6789") },
  { id: "zuri", name: "Zuri", variant: "classic", url: tex("eee522611005acf256dbd152e992c60c0bb7978cb0f3127807700e478ad97664") },
];
