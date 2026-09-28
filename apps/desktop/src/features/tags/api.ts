import { invoke } from "@tauri-apps/api/core";

export interface Tag {
  id: string;
  name: string;
  color: string | null;
  itemCount: number;
}

export async function createTag(name: string, color?: string): Promise<Tag> {
  return invoke<Tag>("create_tag", { request: { name, color } });
}

export async function listTags(): Promise<Tag[]> {
  return invoke<Tag[]>("list_tags");
}

/** Renames a tag globally (affects every item carrying it). */
export async function renameTag(tagId: string, name: string): Promise<void> {
  return invoke<void>("rename_tag", { request: { tagId, name } });
}

/** Deletes a tag globally (removes it from every item carrying it). */
export async function deleteTag(tagId: string): Promise<void> {
  return invoke<void>("delete_tag", { tagId });
}

export async function addTagToItem(itemId: string, tagId: string): Promise<void> {
  return invoke<void>("add_tag_to_item", { itemId, tagId });
}

export async function removeTagFromItem(itemId: string, tagId: string): Promise<void> {
  return invoke<void>("remove_tag_from_item", { itemId, tagId });
}

export async function getItemTags(itemId: string): Promise<Tag[]> {
  return invoke<Tag[]>("get_item_tags", { itemId });
}
