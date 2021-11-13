use bevy::prelude::*;

use crate::serialize::{BuildableRef, Buildables};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SlotState {
    Normal,
    Selected,
    Empty,
}

impl SlotState {
    pub fn from_data(count: u32, selected: bool) -> SlotState {
        if count == 0 {
            SlotState::Empty
        } else {
            if selected {
                SlotState::Selected
            } else {
                SlotState::Normal
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Buildable {
    /// Display name.
    name: String,
    /// Weight.
    weight: f32,
    /// Is the buildable stackable?
    stackable: bool,
    /// Handle to the 3D model.
    mesh: Handle<Mesh>,
    /// Handle to the material of the 3D model.
    material: Handle<StandardMaterial>,
    /// Handle to the frame material in default state.
    frame_material: Handle<ColorMaterial>,
    /// Handle to the frame material in selected state.
    frame_material_selected: Handle<ColorMaterial>,
    /// Handle to the frame material in empty state.
    frame_material_empty: Handle<ColorMaterial>,
}

impl Buildable {
    pub fn new(
        name: &str,
        weight: f32,
        stackable: bool,
        mesh: Handle<Mesh>,
        material: Handle<StandardMaterial>,
        frame_material: Handle<ColorMaterial>,
        frame_material_selected: Handle<ColorMaterial>,
        frame_material_empty: Handle<ColorMaterial>,
    ) -> Self {
        Buildable {
            name: name.to_owned(),
            weight,
            stackable,
            mesh,
            material,
            frame_material,
            frame_material_selected,
            frame_material_empty,
        }
    }

    /// Get the frame material for the given state, inferred from the item count and selection state.
    pub fn get_frame_material(&self, state: &SlotState) -> Handle<ColorMaterial> {
        match state {
            SlotState::Normal => self.frame_material.clone(),
            SlotState::Empty => self.frame_material_empty.clone(),
            SlotState::Selected => self.frame_material_selected.clone(),
        }
    }

    pub fn weight(&self) -> f32 {
        self.weight
    }

    pub fn mesh(&self) -> &Handle<Mesh> {
        &self.mesh
    }

    pub fn material(&self) -> &Handle<StandardMaterial> {
        &self.material
    }
}

#[derive(Debug, Clone)]
pub struct Slot {
    bref: BuildableRef,
    count: u32,
}

impl Slot {
    pub fn new(bref: BuildableRef, count: u32) -> Self {
        Slot { bref, count }
    }

    pub fn bref(&self) -> &BuildableRef {
        &self.bref
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn pop_item(&mut self) -> Option<BuildableRef> {
        if self.count > 0 {
            self.count -= 1;
            trace!(
                "Removed 1 item from slot '{}', left: {}",
                self.bref.0,
                self.count
            );
            Some(self.bref.clone())
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[derive(Debug, Clone)]
pub struct Inventory {
    slots: Vec<Slot>,
    selected_index: usize,
    root_node: Option<Entity>,
}

impl Inventory {
    pub fn new() -> Inventory {
        Inventory {
            slots: vec![],
            selected_index: 0,
            root_node: None,
        }
    }

    pub fn set_slots<I>(&mut self, slots: I)
    where
        I: IntoIterator<Item = Slot>,
    {
        self.slots = slots.into_iter().collect();
        let slot_count = self.slots.len();
        self.selected_index = if slot_count > 0 {
            self.selected_index.clamp(0, slot_count)
        } else {
            0
        };
    }

    pub fn add_slot(&mut self, bref: BuildableRef, count: u32) -> &Slot {
        self.slots.push(Slot { bref, count });
        self.slots.last().as_ref().unwrap()
    }

    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    pub fn slot(&self, index: u32) -> Option<&Slot> {
        let index = index as usize;
        if index < self.slots.len() {
            Some(&self.slots[index])
        } else {
            None
        }
    }

    pub fn slot_mut(&mut self, index: u32) -> Option<&mut Slot> {
        let index = index as usize;
        if index < self.slots.len() {
            Some(&mut self.slots[index])
        } else {
            None
        }
    }

    pub fn selected_slot(&self) -> Option<&Slot> {
        let num_slots = self.slots.len();
        if num_slots > 0 {
            assert!(self.selected_index < num_slots);
            Some(&self.slots[self.selected_index])
        } else {
            None
        }
    }

    pub fn selected_slot_mut(&mut self) -> Option<&mut Slot> {
        let num_slots = self.slots.len();
        if num_slots > 0 {
            assert!(self.selected_index < num_slots);
            Some(&mut self.slots[self.selected_index])
        } else {
            None
        }
    }

    pub fn select_slot(&mut self, select: &SelectSlot) -> bool {
        let num_slots = self.slots.len();
        if num_slots == 0 {
            return false;
        }
        let old_index = self.selected_index;
        let new_index = match select {
            SelectSlot::Prev => old_index.wrapping_add(num_slots - 1) % num_slots,
            SelectSlot::Next => (old_index + 1) % num_slots,
            SelectSlot::Index(index) => {
                if *index >= num_slots {
                    return false;
                }
                *index
            }
        };
        let changed = new_index != self.selected_index;
        self.selected_index = new_index;
        changed
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().fold(0u32, |acc, x| acc + x.count) == 0
    }

    pub fn find_non_empty_slot_index(&self) -> Option<u32> {
        for (index, item) in self.slots.iter().enumerate() {
            if item.count > 0 {
                return Some(index as u32);
            }
        }
        None
    }
    
    pub fn clear_entities(&mut self, commands: &mut Commands) {
        if let Some(root_node) = self.root_node.take() {
            commands.entity(root_node).despawn_recursive();
        }
    }
}

/// Inventory slot component added to each slot.
struct InventorySlot {
    /// Index of the slot in the [`Inventory`.
    index: u32,
    /// Number of items in the slot.
    count: u32,
    /// Entity owning the text with the number of items.
    text: Entity,
}

impl InventorySlot {
    pub fn new(index: u32, count: u32, text: Entity) -> InventorySlot {
        InventorySlot { index, count, text }
    }
}

/// Event to update the inventory slots.
pub struct UpdateInventorySlots;

pub enum SelectSlot {
    Prev,
    Next,
    Index(usize),
}

/// Event to select a slot in the inventory.
pub struct SelectSlotEvent(pub SelectSlot);

#[derive(Debug, Default, Clone)]
struct UiResources {
    pub font: Handle<Font>,
    pub transparent_material: Handle<ColorMaterial>,
}

impl UiResources {
    pub fn new() -> Self {
        UiResources {
            ..Default::default()
        }
    }
}

/// Event to regenerate the UI of the inventory.
pub struct RegenerateInventoryUiEvent;

fn setup(
    asset_server: Res<AssetServer>,
    mut materials2d: ResMut<Assets<ColorMaterial>>,
    mut ui_resouces: ResMut<UiResources>,
) {
    let font = asset_server.load("fonts/mochiy_pop_one/MochiyPopOne-Regular.ttf");
    let transparent_material = materials2d.add(Color::NONE.into());
    *ui_resouces = UiResources {
        font,
        transparent_material,
    }
}

fn update_slots(
    buildables: Res<Buildables>,
    mut inventory: ResMut<Inventory>,
    mut ev_select_slot: EventReader<SelectSlotEvent>,
    mut ev_update_slots: EventReader<UpdateInventorySlots>,
    mut slot_query: Query<(&mut InventorySlot, &mut Handle<ColorMaterial>, &Children)>,
    mut text_query: Query<&mut Text>,
) {
    // Consume all events in order and calculate the new slot index
    let mut changed = false;
    for ev in ev_select_slot.iter() {
        changed = changed || inventory.select_slot(&ev.0);
    }

    // Update all inventory slots
    if changed || ev_update_slots.iter().count() > 0 {
        let selected_index = inventory.selected_index;
        trace!("UpdateInventorySlots: sel={}", selected_index);
        for (mut slot, mut material, children) in slot_query.iter_mut() {
            let mut text = text_query.get_mut(children[0]).unwrap();
            let index = slot.index;
            if let Some(slot_def) = inventory.slot(index) {
                let bref = slot_def.bref();
                let count = slot_def.count();
                if let Some(buildable) = buildables.get(bref) {
                    slot.count = count;
                    text.sections[0].value = format!("x{}", count).to_string();
                    trace!("-- slot: idx={} cnt={}", index, count);
                    let slot_state = SlotState::from_data(count, index == selected_index as u32);
                    *material = buildable.get_frame_material(&slot_state);
                }
            }
        }
    }
}

fn regenerate_ui(
    mut commands: Commands,
    mut ev_regen_ui: EventReader<RegenerateInventoryUiEvent>,
    asset_server: Res<AssetServer>,
    mut inventory: ResMut<Inventory>,
    buildables: Res<Buildables>,
    ui_resouces: Res<UiResources>,
) {
    if let Some(ev) = ev_regen_ui.iter().last() {
        trace!("regenerate_ui() -- GOT EVENT!");
        if let Some(root) = inventory.root_node {
            trace!("Despawning inventory UI rooted at {:?}", root);
            commands.entity(root).despawn_recursive();
        } else {
            trace!("Inventory UI was empty.");
        }
        inventory.root_node = Some(
            commands
                .spawn_bundle(NodeBundle {
                    style: Style {
                        size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                        justify_content: JustifyContent::FlexEnd,
                        ..Default::default()
                    },
                    material: ui_resouces.transparent_material.clone(),
                    ..Default::default()
                })
                .with_children(|parent| {
                    if inventory.slots().len() == 0 {
                        error!("Empty inventory!");
                        return;
                    }
                    trace!(
                        "Generating inventory with {} slots",
                        inventory.slots().len()
                    );
                    let mut xpos = 100.0 + 200.0 * (inventory.slots().len() - 1) as f32;
                    let font = ui_resouces.font.clone();
                    for (index, slot) in inventory.slots().iter().enumerate() {
                        let bref = slot.bref();
                        let count = slot.count();
                        trace!("[#{}] {} x {}", index, bref.0, count);
                        if let Some(buildable) = buildables.get(bref) {
                            // Item slot with frame and item image
                            let mut frame = parent.spawn_bundle(NodeBundle {
                                style: Style {
                                    size: Size::new(Val::Px(128.0), Val::Px(128.0)),
                                    position_type: PositionType::Absolute,
                                    position: Rect {
                                        bottom: Val::Px(100.0),
                                        right: Val::Px(xpos),
                                        ..Default::default()
                                    },

                                    // I expect one of these to center the text in the node
                                    align_content: AlignContent::Center,
                                    align_items: AlignItems::Center,
                                    align_self: AlignSelf::Center,

                                    // this line aligns the content
                                    justify_content: JustifyContent::Center,
                                    ..Default::default()
                                },
                                material: buildable
                                    .get_frame_material(&SlotState::from_data(count, index == 0)),
                                ..Default::default()
                            });
                            let text = frame
                                .with_children(|parent| {
                                    // Item count in slot
                                    parent.spawn_bundle(TextBundle {
                                        text: Text::with_section(
                                            format!("x{}", count).to_string(),
                                            TextStyle {
                                                font: font.clone(),
                                                font_size: 90.0,
                                                color: Color::rgb_u8(111, 188, 165),
                                            },
                                            Default::default(), // TextAlignment
                                        ),
                                        ..Default::default()
                                    });
                                })
                                .id();
                            frame.insert(InventorySlot::new(index as u32, count, text));
                            xpos -= 200.0;
                        } else {
                            error!("Unknown buildable reference {:?}", bref);
                        }
                    }
                })
                .id(),
        );
        trace!(
            "Created slot widget hierarchy from root {:?}",
            inventory.root_node
        );
    }
}

/// Plugin for managing the inventory while a level is being played.
pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut AppBuilder) {
        // Add Inventory resource and SelectSlotEvent event
        app.insert_resource(Inventory::new())
            .insert_resource(UiResources::new())
            .add_event::<RegenerateInventoryUiEvent>()
            .add_event::<SelectSlotEvent>()
            .add_event::<UpdateInventorySlots>();

        // Add system to manage the inventory
        app.add_startup_system(setup.system())
            .add_system(update_slots.system())
            .add_system(regenerate_ui.system());
    }
}
