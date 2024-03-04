use bevy::prelude::*;
use bevy_yoetz::prelude::*;
use turborand::rng::Rng;
use turborand::TurboRand;

use self::examples_lib::debug_text::{ExampleDebugText, ExampleDebugTextPlugin};

mod examples_lib;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(YoetzPlugin::<EnemyBehavior>::default())
        .add_plugins(ExampleDebugTextPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                control_player,
                (
                    enemies_idle,
                    enemies_detect_player,
                    enemies_in_distance_for_circling,
                )
                    .in_set(YoetzSystemSet::Suggest),
                (
                    enemies_do_nothing,
                    enemies_follow_player,
                    enemies_circle_player,
                )
                    .in_set(YoetzSystemSet::Act),
            ),
        )
        .add_systems(
            Update,
            update_enemies_debug_text.in_set(YoetzSystemSet::Act),
        )
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Enemy;

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle {
        transform: Transform::from_scale(0.05 * Vec3::ONE),
        ..Default::default()
    });

    commands.spawn((
        Player,
        SpriteBundle {
            sprite: Sprite {
                color: Color::YELLOW,
                custom_size: Some(Vec2::new(1.0, 1.0)),
                ..Default::default()
            },
            ..Default::default()
        },
    ));

    commands.spawn((
        Enemy,
        YoetzAdvisor::<EnemyBehavior>::new(2.0),
        SpriteBundle {
            transform: Transform::from_xyz(-5.0, 5.0, 0.0),
            sprite: Sprite {
                color: Color::RED,
                custom_size: Some(Vec2::new(1.0, 1.0)),
                ..Default::default()
            },
            ..Default::default()
        },
        ExampleDebugText::new(Color::WHITE),
    ));
}

fn control_player(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>,
    time: Res<Time>,
) {
    let mut direction = Vec3::ZERO;

    if keyboard.pressed(KeyCode::ArrowUp) {
        direction += Vec3::Y;
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        direction -= Vec3::Y;
    }
    if keyboard.pressed(KeyCode::ArrowLeft) {
        direction -= Vec3::X;
    }
    if keyboard.pressed(KeyCode::ArrowRight) {
        direction += Vec3::X;
    }

    for mut player_transform in query.iter_mut() {
        player_transform.translation += 10.0 * time.delta_seconds() * direction;
    }
}

#[derive(YoetzSuggestion)]
#[yoetz(key_enum(derive(Debug)), strategy_structs(derive(Debug)))]
enum EnemyBehavior {
    Idle,
    Chase {
        #[yoetz(key)]
        target_entity: Entity,
        #[yoetz(input)]
        vec_to_target: Vec2,
    },
    Circle {
        #[yoetz(key)]
        target_entity: Entity,
        #[yoetz(input)]
        vec_to_target: Vec2,
        #[yoetz(state)]
        go_counter_clockwise: bool,
    },
}

fn enemies_idle(mut query: Query<&mut YoetzAdvisor<EnemyBehavior>, With<Enemy>>) {
    for mut advisor in query.iter_mut() {
        advisor.suggest(5.0, EnemyBehavior::Idle);
    }
}

fn enemies_detect_player(
    mut enemies_query: Query<(&mut YoetzAdvisor<EnemyBehavior>, &GlobalTransform), With<Enemy>>,
    player_query: Query<(Entity, &GlobalTransform), With<Player>>,
) {
    for (mut advisor, enemy_transform) in enemies_query.iter_mut() {
        let enemy_position = enemy_transform.translation();
        for (player_entity, player_transform) in player_query.iter() {
            let player_position = player_transform.translation();
            let vec_to_player = (player_position - enemy_position).truncate();
            advisor.suggest(
                12.0 - vec_to_player.length(),
                EnemyBehavior::Chase {
                    target_entity: player_entity,
                    vec_to_target: vec_to_player,
                },
            );
        }
    }
}

fn enemies_in_distance_for_circling(
    mut enemies_query: Query<(&mut YoetzAdvisor<EnemyBehavior>, &GlobalTransform), With<Enemy>>,
    player_query: Query<(Entity, &GlobalTransform), With<Player>>,
    rand: Local<Rng>,
) {
    for (mut advisor, enemy_transform) in enemies_query.iter_mut() {
        let enemy_position = enemy_transform.translation();
        for (player_entity, player_transform) in player_query.iter() {
            let player_position = player_transform.translation();
            let vec_to_player = (player_position - enemy_position).truncate();
            if vec_to_player.length() < 4.0 {
                advisor.suggest(
                    100.0,
                    EnemyBehavior::Circle {
                        target_entity: player_entity,
                        vec_to_target: vec_to_player,
                        go_counter_clockwise: rand.bool(),
                    },
                );
            }
        }
    }
}

fn enemies_do_nothing(query: Query<&EnemyBehaviorIdle>) {
    for _ in query.iter() {
        // TODO: change to random walk?
    }
}

fn enemies_follow_player(mut query: Query<(&EnemyBehaviorChase, &mut Transform)>, time: Res<Time>) {
    for (chase, mut transform) in query.iter_mut() {
        let Some(direction) = chase.vec_to_target.try_normalize() else {
            continue;
        };
        transform.translation += 5.0 * time.delta_seconds() * direction.extend(0.0);
    }
}

fn enemies_circle_player(
    mut query: Query<(&EnemyBehaviorCircle, &mut Transform)>,
    time: Res<Time>,
) {
    for (circle, mut transform) in query.iter_mut() {
        let Some(direction) = circle.vec_to_target.try_normalize() else {
            continue;
        };
        let direction = if circle.go_counter_clockwise {
            direction.perp()
        } else {
            -direction.perp()
        };
        transform.translation -= 5.0 * time.delta_seconds() * direction.extend(0.0);
    }
}

#[allow(clippy::type_complexity)]
fn update_enemies_debug_text(
    mut query: Query<(
        &mut ExampleDebugText,
        &YoetzAdvisor<EnemyBehavior>,
        Option<&EnemyBehaviorIdle>,
        Option<&EnemyBehaviorChase>,
        Option<&EnemyBehaviorCircle>,
    )>,
) {
    for (mut debug_text, advisor, idle, chase, circle) in query.iter_mut() {
        use std::fmt::Write;
        let mut text = String::default();

        fn write_if_some(sink: &mut String, value: Option<impl std::fmt::Debug>) {
            if let Some(value) = value {
                writeln!(sink, "{value:#?}").unwrap();
            }
        }

        write_if_some(&mut text, advisor.active_key().as_ref());
        write_if_some(&mut text, idle);
        write_if_some(&mut text, chase);
        write_if_some(&mut text, circle);

        debug_text.text = text;
    }
}
