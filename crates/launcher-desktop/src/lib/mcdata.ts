// A curated set of common Minecraft item ids for the inventory picker.
// You can also type any id (including modpack items like "create:cogwheel").

export const VANILLA_ITEMS: string[] = [
  // Tools & weapons
  "minecraft:netherite_sword", "minecraft:diamond_sword", "minecraft:iron_sword", "minecraft:bow",
  "minecraft:crossbow", "minecraft:trident", "minecraft:mace", "minecraft:shield",
  "minecraft:netherite_pickaxe", "minecraft:diamond_pickaxe", "minecraft:iron_pickaxe",
  "minecraft:netherite_axe", "minecraft:diamond_axe", "minecraft:netherite_shovel",
  "minecraft:diamond_shovel", "minecraft:netherite_hoe", "minecraft:fishing_rod", "minecraft:flint_and_steel",
  "minecraft:shears", "minecraft:spyglass", "minecraft:brush",
  // Armor
  "minecraft:netherite_helmet", "minecraft:netherite_chestplate", "minecraft:netherite_leggings",
  "minecraft:netherite_boots", "minecraft:diamond_helmet", "minecraft:diamond_chestplate",
  "minecraft:diamond_leggings", "minecraft:diamond_boots", "minecraft:elytra", "minecraft:turtle_helmet",
  // Valuables
  "minecraft:diamond", "minecraft:netherite_ingot", "minecraft:netherite_scrap", "minecraft:emerald",
  "minecraft:gold_ingot", "minecraft:iron_ingot", "minecraft:copper_ingot", "minecraft:ancient_debris",
  "minecraft:nether_star", "minecraft:totem_of_undying", "minecraft:experience_bottle", "minecraft:dragon_egg",
  // Food
  "minecraft:golden_apple", "minecraft:enchanted_golden_apple", "minecraft:bread", "minecraft:cooked_beef",
  "minecraft:cooked_porkchop", "minecraft:golden_carrot", "minecraft:cake", "minecraft:apple",
  // Utility
  "minecraft:ender_pearl", "minecraft:ender_eye", "minecraft:ender_chest", "minecraft:shulker_box",
  "minecraft:bundle", "minecraft:bucket", "minecraft:water_bucket", "minecraft:lava_bucket",
  "minecraft:milk_bucket", "minecraft:compass", "minecraft:clock", "minecraft:map", "minecraft:name_tag",
  "minecraft:lead", "minecraft:saddle", "minecraft:firework_rocket", "minecraft:tnt",
  // Redstone
  "minecraft:redstone", "minecraft:redstone_torch", "minecraft:repeater", "minecraft:comparator",
  "minecraft:piston", "minecraft:sticky_piston", "minecraft:observer", "minecraft:hopper",
  "minecraft:dropper", "minecraft:dispenser", "minecraft:lever", "minecraft:redstone_block",
  // Building blocks
  "minecraft:stone", "minecraft:cobblestone", "minecraft:oak_log", "minecraft:oak_planks",
  "minecraft:glass", "minecraft:obsidian", "minecraft:bedrock", "minecraft:dirt", "minecraft:sand",
  "minecraft:gravel", "minecraft:netherrack", "minecraft:end_stone", "minecraft:sea_lantern",
  "minecraft:glowstone", "minecraft:bookshelf", "minecraft:crafting_table", "minecraft:furnace",
  "minecraft:enchanting_table", "minecraft:anvil", "minecraft:beacon", "minecraft:chest",
  "minecraft:barrel", "minecraft:smithing_table", "minecraft:grindstone", "minecraft:lodestone",
  "minecraft:respawn_anchor", "minecraft:netherite_block", "minecraft:diamond_block", "minecraft:gold_block",
  "minecraft:iron_block", "minecraft:emerald_block", "minecraft:torch", "minecraft:lantern",
  // Misc
  "minecraft:stick", "minecraft:string", "minecraft:gunpowder", "minecraft:blaze_rod", "minecraft:blaze_powder",
  "minecraft:slime_ball", "minecraft:bone", "minecraft:gold_nugget", "minecraft:iron_nugget",
  "minecraft:leather", "minecraft:feather", "minecraft:book", "minecraft:enchanted_book", "minecraft:paper",
  "minecraft:wheat", "minecraft:coal", "minecraft:charcoal", "minecraft:arrow", "minecraft:spectral_arrow",
  "minecraft:music_disc_pigstep", "minecraft:heart_of_the_sea", "minecraft:nautilus_shell",
  "minecraft:phantom_membrane", "minecraft:echo_shard", "minecraft:amethyst_shard",
];

export interface EnchantDef {
  id: string;
  name: string;
  max: number;
}

export const ENCHANTMENTS: EnchantDef[] = [
  { id: "minecraft:protection", name: "Protection", max: 4 },
  { id: "minecraft:fire_protection", name: "Fire Protection", max: 4 },
  { id: "minecraft:blast_protection", name: "Blast Protection", max: 4 },
  { id: "minecraft:projectile_protection", name: "Projectile Protection", max: 4 },
  { id: "minecraft:feather_falling", name: "Feather Falling", max: 4 },
  { id: "minecraft:respiration", name: "Respiration", max: 3 },
  { id: "minecraft:aqua_affinity", name: "Aqua Affinity", max: 1 },
  { id: "minecraft:thorns", name: "Thorns", max: 3 },
  { id: "minecraft:depth_strider", name: "Depth Strider", max: 3 },
  { id: "minecraft:frost_walker", name: "Frost Walker", max: 2 },
  { id: "minecraft:soul_speed", name: "Soul Speed", max: 3 },
  { id: "minecraft:swift_sneak", name: "Swift Sneak", max: 3 },
  { id: "minecraft:sharpness", name: "Sharpness", max: 5 },
  { id: "minecraft:smite", name: "Smite", max: 5 },
  { id: "minecraft:bane_of_arthropods", name: "Bane of Arthropods", max: 5 },
  { id: "minecraft:knockback", name: "Knockback", max: 2 },
  { id: "minecraft:fire_aspect", name: "Fire Aspect", max: 2 },
  { id: "minecraft:looting", name: "Looting", max: 3 },
  { id: "minecraft:sweeping_edge", name: "Sweeping Edge", max: 3 },
  { id: "minecraft:efficiency", name: "Efficiency", max: 5 },
  { id: "minecraft:silk_touch", name: "Silk Touch", max: 1 },
  { id: "minecraft:unbreaking", name: "Unbreaking", max: 3 },
  { id: "minecraft:fortune", name: "Fortune", max: 3 },
  { id: "minecraft:power", name: "Power", max: 5 },
  { id: "minecraft:punch", name: "Punch", max: 2 },
  { id: "minecraft:flame", name: "Flame", max: 1 },
  { id: "minecraft:infinity", name: "Infinity", max: 1 },
  { id: "minecraft:luck_of_the_sea", name: "Luck of the Sea", max: 3 },
  { id: "minecraft:lure", name: "Lure", max: 3 },
  { id: "minecraft:loyalty", name: "Loyalty", max: 3 },
  { id: "minecraft:impaling", name: "Impaling", max: 5 },
  { id: "minecraft:riptide", name: "Riptide", max: 3 },
  { id: "minecraft:channeling", name: "Channeling", max: 1 },
  { id: "minecraft:multishot", name: "Multishot", max: 1 },
  { id: "minecraft:quick_charge", name: "Quick Charge", max: 3 },
  { id: "minecraft:piercing", name: "Piercing", max: 4 },
  { id: "minecraft:density", name: "Density", max: 5 },
  { id: "minecraft:breach", name: "Breach", max: 4 },
  { id: "minecraft:wind_burst", name: "Wind Burst", max: 3 },
  { id: "minecraft:mending", name: "Mending", max: 1 },
  { id: "minecraft:vanishing_curse", name: "Curse of Vanishing", max: 1 },
  { id: "minecraft:binding_curse", name: "Curse of Binding", max: 1 },
];
