extern crate nalgebra_glm as glm;
extern crate quipi;

pub use quipi::prelude::*;
use quipi::{
    asset_manager::assets::{camera::OrthographicCameraParams, RCamera2D, RShader},
    core::prelude::{
        random::Random,
        trig::{magnitude2d_squared, rotate2d},
        Countdown, Interval,
    },
    ecs::prelude::components::CTransform2D,
    gfx::prelude::{ShaderUniforms, SpriteRenderer, SPRITE_FRAG, SPRITE_VERT},
    schemas::sprite::TextureAtlas,
};

use qp_ecs::components::*;
use sdl2::{event::WindowEvent, keyboard::Keycode};

pub static WIDTH: u32 = 1600;
pub static HEIGHT: u32 = 900;

pub fn run() -> Result<(), QPError> {
    let mut app = App::init("Space Shooter", WIDTH, HEIGHT, 348756)?;

    app.world.registry.asset_manager.load_asset(
        "shader",
        RShader::from_str(
            SPRITE_VERT,
            SPRITE_FRAG,
            vec![
                ShaderUniforms::ViewMatrix("view".into()),
                ShaderUniforms::ProjectionMatrix("projection".into()),
            ],
        )?,
    )?;

    app.world.registry.asset_manager.load_asset(
        "space_tilesheet",
        qp_assets::RTexture {
            texture: qp_gfx::texture::from_image("assets/textures/space.png")?,
            texture_dims: glm::vec2(8.0, 6.0),
        },
    )?;

    let game = GameController::new(&mut app)?;
    app.register_controller(game);

    let renderer = SpriteRenderer::new(&mut app.world.registry, "camera", "shader")?;
    app.register_renderer(renderer);

    app.run((0.1, 0.1, 0.1, 1.0))
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Space Shooter ended unexpectedly: {}", e);
    }
}

pub struct GameController {
    rand: Random,

    score: Score,
    score_interval: Interval,
    game_over_text: GameOver,
    ship: Ship,
    bullets: Vec<Bullet>,
    asteroids: Vec<Asteroid>,
    stars: Vec<Star>,

    asteroid_spawn_interval: Interval,
    bullet_spawn_interval: Interval,
    star_spawn_interval: Interval,

    game_over: bool,
    firing: bool,
}

impl GameController {
    pub fn new(app: &mut App) -> Result<Self, QPError> {
        let font = app
            .world
            .registry
            .asset_manager
            .load_asset("Poppins-Regular", qp_assets::RFont::new("Poppins-Regular")?)?;

        let ship = Ship::new(&mut app.world)?;
        let camera = Camera::new(&mut app.world, ship.index)?;
        let score = Score::new(font)?;
        let game_over_text = GameOver::new(font)?;

        app.register_controller(camera);

        if cfg!(debug_assertions) {
            let text = DebugInfoText::new(font)?;
            app.register_controller(text);
        }

        let rand = Random::from_seed(1234);

        Ok(Self {
            rand,
            score,
            game_over_text,
            ship,
            bullets: vec![],
            asteroids: vec![],
            stars: vec![],
            score_interval: Interval::new(0.5),
            asteroid_spawn_interval: Interval::new(0.5),
            bullet_spawn_interval: Interval::new(0.2),
            star_spawn_interval: Interval::new(0.002),
            game_over: false,
            firing: false,
        })
    }

    fn reset(&mut self, registry: &mut GlobalRegistry) {
        for bullet in self.bullets.iter() {
            registry.entity_manager.set_to_delete(bullet.index);
        }
        for asteroid in self.asteroids.iter() {
            registry.entity_manager.set_to_delete(asteroid.index);
        }

        self.bullets.clear();
        self.asteroids.clear();

        self.asteroid_spawn_interval.check();
        self.bullet_spawn_interval.check();

        self.score.score = 0;
        self.game_over = false;
    }

    fn spawn_bullet(&mut self, registry: &mut GlobalRegistry) -> Result<(), QPError> {
        if !self.bullet_spawn_interval.check() || !self.firing {
            return Ok(());
        }

        let Some(ship) = registry
            .entity_manager
            .get::<CTransform2D>(&self.ship.index)
        else {
            return Ok(());
        };
        let direction = ship.direction();
        let position = ship.translate + (direction * 28.0);
        let bullet = Bullet::new(registry, position, direction, ship.rotate);

        self.bullets.push(bullet?);

        Ok(())
    }

    fn spawn_asteroid(&mut self, world: &mut World) -> Result<(), QPError> {
        if !self.asteroid_spawn_interval.check() {
            return Ok(());
        }

        let (_x, _y, width, height) = world.viewport.get_dimensions();

        let ship = world
            .registry
            .entity_manager
            .get::<CTransform2D>(&self.ship.index)
            .unwrap();

        let ship_pos = ship.translate;
        let offset = 100.0;
        let pos_local = glm::vec2((width / 2) as f32 + offset, (height / 2) as f32 + offset);
        let pos_local = rotate2d(&pos_local, self.rand.random() * 2.0 * glm::pi::<f32>());
        let pos = ship_pos + pos_local;

        let asteroid = Asteroid::new(
            &mut world.registry,
            ship_pos,
            pos,
            (self.rand.random() + 1.0) * 2.0,
            self.rand.random() * 2.0 * glm::pi::<f32>(),
            &mut world.rand,
        )?;
        self.asteroids.push(asteroid);

        Ok(())
    }

    fn spawn_star(&mut self, world: &mut World) -> Result<(), QPError> {
        if !self.star_spawn_interval.check() {
            return Ok(());
        }

        let ship_transform = world
            .registry
            .entity_manager
            .get::<CTransform2D>(&self.ship.index)
            .unwrap();

        let ship_pos = ship_transform.translate;
        let star = Star::new(world, ship_pos)?;

        self.stars.push(star);

        Ok(())
    }
}

impl Controller for GameController {
    fn update(&mut self, world: &mut World) -> FrameResult {
        for event in world.events.iter() {
            match event {
                Event::Quit { .. } => {
                    return FrameResult::Quit;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    return FrameResult::Quit;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Space),
                    repeat: false,
                    ..
                } => {
                    if !self.game_over {
                        self.firing = true;
                    }
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Return),
                    repeat: false,
                    ..
                } => {
                    if self.game_over {
                        self.reset(&mut world.registry);
                    }
                }
                Event::KeyUp {
                    keycode: Some(Keycode::Space),
                    ..
                } => {
                    self.firing = false;
                }
                _ => (),
            };
        }

        self.score.update(world);

        if self.game_over {
            self.game_over_text.update(world);

            return FrameResult::None;
        }

        for asteroid in self.asteroids.iter_mut() {
            if !asteroid.alive {
                continue;
            }

            // check for collision with ship
            let ship_transform = world
                .registry
                .entity_manager
                .get::<CTransform2D>(&self.ship.index)
                .unwrap()
                .clone();

            if asteroid.check_collision(&mut world.registry, &ship_transform, 32.0) {
                self.game_over = true;
            };

            // check for collision with bullet
            for bullet in self.bullets.iter_mut() {
                if !bullet.alive {
                    continue;
                }

                let bullet_transform = world
                    .registry
                    .entity_manager
                    .get::<CTransform2D>(&bullet.index)
                    .unwrap()
                    .clone();

                if asteroid.check_collision(&mut world.registry, &bullet_transform, 16.0) {
                    self.score.score += 5;
                    bullet.destroy(&mut world.registry);
                };
            }

            asteroid.update(world);
        }

        for bullet in self.bullets.iter_mut() {
            bullet.update(world);
        }

        for star in self.stars.iter_mut() {
            star.update(world);
        }

        self.spawn_asteroid(world).unwrap();
        self.spawn_bullet(&mut world.registry).unwrap();
        self.spawn_star(world).unwrap();

        self.ship.update(world);

        if self.score_interval.check() {
            self.score.score += 1;
        }

        FrameResult::None
    }
}

pub struct Camera {
    ship: VersionedIndex,
    id: u64,
}

impl Camera {
    pub fn new(world: &mut World, ship: VersionedIndex) -> Result<Self, QPError> {
        let mut camera = RCamera2D::new(
            OrthographicCameraParams {
                right: WIDTH as f32,
                top: HEIGHT as f32,
                ..OrthographicCameraParams::default()
            },
            1.0,
            CTransform2D::default(),
        );
        let transform = world
            .registry
            .entity_manager
            .get::<CTransform2D>(&ship)
            .unwrap();

        camera.follow(transform, 10.0, 1.0);

        let id = world.registry.asset_manager.load_asset("camera", camera)?;

        Ok(Self { ship, id })
    }
}

impl Controller for Camera {
    fn update(&mut self, world: &mut World) -> FrameResult {
        for event in world.events.iter() {
            match event {
                Event::Window {
                    win_event: WindowEvent::Resized(w, h),
                    ..
                } => {
                    world.viewport.set_dimensions(0, 0, *w, *h);

                    if let Some(camera) = world.registry.asset_manager.get_mut::<RCamera2D>(self.id)
                    {
                        camera.params.right = *w as f32;
                        camera.params.top = *h as f32;

                        camera.view = camera.calc_view_matrix();
                        camera.projection = camera.calc_projection_matrix();
                    }
                }
                _ => (),
            };
        }

        let Some(transform) = world
            .registry
            .entity_manager
            .get::<CTransform2D>(&self.ship)
        else {
            return FrameResult::None;
        };
        let Some(camera) = world.registry.asset_manager.get_mut::<RCamera2D>(self.id) else {
            return FrameResult::None;
        };

        camera.follow(transform, 30.0, 0.06);

        FrameResult::None
    }
}

pub struct Ship {
    pub index: VersionedIndex,
    particle_system: ParticleSystem,

    mouse_pos: glm::Vec2,
    acceleration: f32,
    max_velocity: f32,
    thrust: bool,
}

impl Ship {
    pub fn new(world: &mut World) -> Result<Self, QPError> {
        let texture_id = world
            .registry
            .asset_manager
            .get_asset_id("space_tilesheet")
            .ok_or(QPError::SpriteTextureDoesntExist)?;

        let texture = world
            .registry
            .asset_manager
            .get::<qp_assets::RTexture>(texture_id)
            .ok_or(QPError::SpriteTextureDoesntExist)?;

        let index = EntityBuilder::create(&mut world.registry.entity_manager)
            .with(CTag {
                tag: "ship".to_string(),
            })
            .with(CVelocity2D { x: 0.0, y: 0.0 })
            .with(CTransform2D {
                translate: glm::vec2(0.0, 0.0),
                ..CTransform2D::default()
            })
            .with(CSprite::new(
                &CQuad {
                    width: 64.0,
                    height: 64.0,
                    center_x: 0.0,
                    center_y: 0.0,
                },
                Some(glm::vec4(0.8, 0.2, 0.0, 1.0)),
                Some(TextureAtlas {
                    texture: texture_id,
                    texture_dims: texture.texture_dims,
                    active_texture: glm::vec2(1.0, 3.0),
                }),
            ))
            .build();

        let particle_system = ParticleSystem::new(index);

        Ok(Self {
            index,
            acceleration: 100.0,
            max_velocity: 500.0,
            thrust: false,
            mouse_pos: glm::vec2(0.0, 0.0),
            particle_system,
        })
    }

    fn rotate_ship(&mut self, world: &mut World) {
        let (_x, _y, width, height) = world.viewport.get_dimensions();
        let x = self.mouse_pos.x - width as f32 / 2.0;
        let y = (self.mouse_pos.y - height as f32 / 2.0) * -1.0;
        let angle = qp_core::trig::angle(&glm::vec3(0.0, 1.0, 0.0), &glm::vec3(x, y, 0.0));

        if let Some(transform) = world
            .registry
            .entity_manager
            .get_mut::<CTransform2D>(&self.index)
        {
            transform.rotate = match x > 0.0 {
                true => {
                    let angle = (2.0 * glm::pi::<f32>()) - angle;

                    angle
                }
                false => angle,
            };
        }
    }
}

impl Controller for Ship {
    fn update(&mut self, world: &mut World) -> FrameResult {
        for event in world.events.iter() {
            match event {
                Event::MouseMotion { x, y, .. } => {
                    self.mouse_pos = glm::vec2(*x as f32, *y as f32);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::W),
                    repeat: false,
                    ..
                } => {
                    self.thrust = true;
                }
                Event::KeyUp {
                    keycode: Some(Keycode::W),
                    repeat: false,
                    ..
                } => {
                    self.thrust = false;
                }
                _ => (),
            };
        }

        self.rotate_ship(world);
        self.particle_system.update(world, self.thrust);

        // calculate velocity based on direction and acceleration
        let Some(velocity) = world
            .registry
            .entity_manager
            .get_mut::<CVelocity2D>(&self.index)
        else {
            return FrameResult::None;
        };

        if !self.thrust && velocity.x <= 0.0 && velocity.y <= 0.0 {
            return FrameResult::None;
        }

        let acceleration = world.delta
            * match self.thrust {
                true => self.acceleration,
                false => -self.acceleration,
            };

        velocity.x = (velocity.x + acceleration).clamp(0.0, self.max_velocity);
        velocity.y = (velocity.y + acceleration).clamp(0.0, self.max_velocity);

        // apply velocity to translation
        let Some(velocity) = world
            .registry
            .entity_manager
            .get::<CVelocity2D>(&self.index)
        else {
            return FrameResult::None;
        };

        let velocity = velocity.clone();

        if let Some(transform) = world
            .registry
            .entity_manager
            .get_mut::<CTransform2D>(&self.index)
        {
            let direction = transform.direction();
            transform.translate.x += velocity.x * world.delta * direction.x;
            transform.translate.y += velocity.y * world.delta * direction.y;
        }

        FrameResult::None
    }
}

pub struct Bullet {
    index: VersionedIndex,
    countdown: Countdown,
    alive: bool,
}

impl Bullet {
    pub fn new(
        registry: &mut GlobalRegistry,
        position: glm::Vec2,
        direction: glm::Vec2,
        angle: f32,
    ) -> Result<Self, QPError> {
        let texture_id = registry
            .asset_manager
            .get_asset_id("space_tilesheet")
            .ok_or(QPError::SpriteTextureDoesntExist)?;

        let texture = registry
            .asset_manager
            .get::<qp_assets::RTexture>(texture_id)
            .ok_or(QPError::SpriteTextureDoesntExist)?;

        let quad = CQuad {
            width: 64.0,
            height: 64.0,
            center_x: 0.0,
            center_y: 0.0,
        };

        let speed = 700.0;

        let index = EntityBuilder::create(&mut registry.entity_manager)
            .with(CTag {
                tag: "bullet".to_string(),
            })
            .with(CTransform2D {
                translate: position,
                rotate: angle,
                scale: glm::vec2(0.7, 0.7),
                ..CTransform2D::default()
            })
            .with(CVelocity2D {
                x: speed * direction.x,
                y: speed * direction.y,
            })
            .with(CSprite::new(
                &quad,
                Some(glm::vec4(0.0, 0.7, 1.0, 1.0)),
                Some(TextureAtlas {
                    texture: texture_id,
                    texture_dims: texture.texture_dims,
                    active_texture: glm::vec2(4.0, 2.0),
                }),
            ))
            .build();

        Ok(Self {
            index,
            countdown: Countdown::new(1.5),
            alive: true,
        })
    }

    pub fn destroy(&mut self, registry: &mut GlobalRegistry) {
        registry.entity_manager.set_to_delete(self.index);
        self.alive = false
    }

    pub fn update(&mut self, world: &mut World) -> bool {
        let time_left = self.countdown.check();

        if time_left == 0.0 {
            self.destroy(&mut world.registry);
            return false;
        }

        let Some(velocity) = world
            .registry
            .entity_manager
            .get::<CVelocity2D>(&self.index)
        else {
            return false;
        };

        let velocity = glm::vec2(velocity.x * world.delta, velocity.y * world.delta);

        if let Some(transform) = world
            .registry
            .entity_manager
            .get_mut::<CTransform2D>(&self.index)
        {
            transform.translate += velocity
        } else {
            return false;
        }

        let sprite = world
            .registry
            .entity_manager
            .get_mut::<CSprite>(&self.index)
            .unwrap();

        sprite.color.w = time_left;

        false
    }
}

struct Asteroid {
    index: VersionedIndex,
    rotation_step: f32,
    scale: f32,
    alive: bool,
    countdown: Countdown,
}

impl Asteroid {
    pub fn new(
        registry: &mut GlobalRegistry,
        ship_pos: glm::Vec2,
        position: glm::Vec2,
        scale: f32,
        rotate: f32,
        rand: &mut Random,
    ) -> Result<Self, QPError> {
        let texture_id = registry
            .asset_manager
            .get_asset_id("space_tilesheet")
            .ok_or(QPError::SpriteTextureDoesntExist)?;

        let texture = registry
            .asset_manager
            .get::<qp_assets::RTexture>(texture_id)
            .ok_or(QPError::SpriteTextureDoesntExist)?;

        let quad = CQuad {
            width: 32.0,
            height: 32.0,
            ..CQuad::default()
        };

        let direction = (ship_pos - position).normalize();

        let index = EntityBuilder::create(&mut registry.entity_manager)
            .with(CTag {
                tag: "asteroid".to_string(),
            })
            .with(CTransform2D {
                translate: position,
                rotate,
                scale: glm::vec2(scale, scale),
                ..CTransform2D::default()
            })
            .with(CVelocity2D {
                x: direction.x * rand.range(150, 300) as f32,
                y: direction.y * rand.range(150, 300) as f32,
            })
            .with(CSprite::new(
                &quad,
                Some(glm::vec4(0.9, 0.9, 0.9, 1.0)),
                Some(TextureAtlas {
                    texture: texture_id,
                    texture_dims: texture.texture_dims,
                    active_texture: glm::vec2(0.0, 1.0),
                }),
            ))
            .build();

        Ok(Self {
            index,
            rotation_step: (rand.random() + 0.7) * 2.0,
            alive: true,
            scale,
            countdown: Countdown::new(30.0),
        })
    }

    pub fn check_collision(
        &mut self,
        registry: &mut GlobalRegistry,
        obj: &CTransform2D,
        obj_radius: f32,
    ) -> bool {
        let transform = registry
            .entity_manager
            .get::<CTransform2D>(&self.index)
            .unwrap();

        let offset = 25.0;
        let threshold = obj_radius + (self.scale * 16.0) - offset;
        if magnitude2d_squared(&transform.translate, &obj.translate) < threshold.powf(2.0) {
            registry.entity_manager.set_to_delete(self.index);
            self.alive = false;

            return true;
        }

        false
    }

    pub fn destroy(&mut self, registry: &mut GlobalRegistry) {
        registry.entity_manager.set_to_delete(self.index);
        self.alive = false;
    }
}

impl Controller for Asteroid {
    fn update(&mut self, world: &mut World) -> FrameResult {
        let time_left = self.countdown.check();

        if time_left == 0.0 {
            self.destroy(&mut world.registry);

            return FrameResult::None;
        }

        let velocity = world
            .registry
            .entity_manager
            .get::<CVelocity2D>(&self.index)
            .unwrap()
            .clone();

        if let Some(transform) = world
            .registry
            .entity_manager
            .get_mut::<CTransform2D>(&self.index)
        {
            transform.translate.x += velocity.x * world.delta;
            transform.translate.y += velocity.y * world.delta;

            transform.rotate += world.delta * self.rotation_step;
        }

        let sprite = world
            .registry
            .entity_manager
            .get_mut::<CSprite>(&self.index)
            .unwrap();

        sprite.color.y = time_left / self.countdown.countdown;
        sprite.color.z = time_left / self.countdown.countdown;
        sprite.color.w = time_left / self.countdown.countdown;

        FrameResult::None
    }
}

struct Star {
    index: VersionedIndex,
    alive: bool,
    countdown: Countdown,
}

impl Star {
    pub fn new(world: &mut World, ship_pos: glm::Vec2) -> Result<Self, QPError> {
        let (_x, _y, width, height) = world.viewport.get_dimensions();
        let x_pos = world
            .rand
            .range(ship_pos.x as i32 - width, ship_pos.x as i32 + width) as f32;

        let y_pos = world
            .rand
            .range(ship_pos.y as i32 - height, ship_pos.y as i32 + height)
            as f32;

        let texture_id = world
            .registry
            .asset_manager
            .get_asset_id("space_tilesheet")
            .ok_or(QPError::SpriteTextureDoesntExist)?;

        let texture = world
            .registry
            .asset_manager
            .get::<qp_assets::RTexture>(texture_id)
            .ok_or(QPError::SpriteTextureDoesntExist)?;

        let quad = CQuad {
            width: 32.0,
            height: 32.0,
            ..CQuad::default()
        };

        let index = EntityBuilder::create(&mut world.registry.entity_manager)
            .with(CTag {
                tag: "asteroid".to_string(),
            })
            .with(CTransform2D {
                translate: glm::vec2(x_pos, y_pos),
                scale: glm::vec2(1.0, 1.0),
                ..CTransform2D::default()
            })
            .with(CSprite::new(
                &quad,
                Some(glm::vec4(1.0, 1.0, 0.8, 1.0)),
                Some(TextureAtlas {
                    texture: texture_id,
                    texture_dims: texture.texture_dims,
                    active_texture: glm::vec2(
                        match world.rand.binary(0.7) {
                            true => 7.0,
                            false => 6.0,
                        },
                        2.0,
                    ),
                }),
            ))
            .build();

        Ok(Self {
            index,
            countdown: Countdown::new(3.0),
            alive: true,
        })
    }
}

impl Controller for Star {
    fn update(&mut self, world: &mut World) -> FrameResult {
        if !self.alive {
            return FrameResult::None;
        }

        let time_left = self.countdown.check();

        if time_left == 0.0 {
            world.registry.entity_manager.set_to_delete(self.index);

            return FrameResult::None;
        }

        let sprite = world
            .registry
            .entity_manager
            .get_mut::<CSprite>(&self.index)
            .unwrap();

        sprite.color.w = time_left / self.countdown.countdown;

        FrameResult::None
    }
}

struct ParticleSystem {
    ship: VersionedIndex,
    particles: Vec<Particle>,
    interval: Interval,
}

impl ParticleSystem {
    pub fn new(ship: VersionedIndex) -> Self {
        Self {
            ship,
            particles: vec![],
            interval: Interval::new(0.02),
        }
    }

    fn spawn(&mut self, world: &mut World) -> Result<(), QPError> {
        if !self.interval.check() {
            return Ok(());
        }

        let Some(ship) = world
            .registry
            .entity_manager
            .get::<CTransform2D>(&self.ship)
        else {
            return Ok(());
        };

        let ship_transform = ship.clone();

        let particle = Particle::new(world, &ship_transform)?;
        self.particles.push(particle);

        Ok(())
    }

    pub fn update(&mut self, world: &mut World, spawn: bool) {
        for particle in self.particles.iter_mut() {
            particle.update(&mut world.registry);
        }

        if spawn {
            self.spawn(world).unwrap();
        }
    }
}

struct Particle {
    index: VersionedIndex,
    alive: bool,
    countdown: Countdown,
}

impl Particle {
    pub fn new(world: &mut World, ship_transform: &CTransform2D) -> Result<Self, QPError> {
        let texture_id = world
            .registry
            .asset_manager
            .get_asset_id("space_tilesheet")
            .ok_or(QPError::SpriteTextureDoesntExist)?;

        let texture = world
            .registry
            .asset_manager
            .get::<qp_assets::RTexture>(texture_id)
            .ok_or(QPError::SpriteTextureDoesntExist)?;

        let quad = CQuad {
            width: 64.0,
            height: 64.0,
            center_x: 0.0,
            center_y: 0.0,
        };

        let offset = ship_transform.direction() * 28.0;
        let scale_factor = world.rand.random() + 1.0;
        let index = EntityBuilder::create(&mut world.registry.entity_manager)
            .with(CTag {
                tag: "particle".to_string(),
            })
            .with(CTransform2D {
                translate: ship_transform.translate - offset,
                rotate: world.rand.random() * 2.0 * glm::pi::<f32>(),
                scale: glm::vec2(scale_factor, scale_factor),
                ..CTransform2D::default()
            })
            .with(CSprite::new(
                &quad,
                Some(glm::vec4(1.0, 0.7, 0.2, 1.0)),
                Some(TextureAtlas {
                    texture: texture_id,
                    texture_dims: texture.texture_dims,
                    active_texture: glm::vec2(4.0, 2.0),
                }),
            ))
            .build();

        Ok(Self {
            index,
            alive: true,
            countdown: Countdown::new(0.4),
        })
    }

    pub fn update(&mut self, registry: &mut GlobalRegistry) {
        if !self.alive {
            return;
        }

        let time_left = self.countdown.check();

        if time_left == 0.0 {
            self.alive = false;
            registry.entity_manager.set_to_delete(self.index);

            return;
        }

        let sprite = registry
            .entity_manager
            .get_mut::<CSprite>(&self.index)
            .unwrap();

        sprite.color.w = time_left;
    }
}

struct Score {
    score: u32,
    font: u64,
}

impl Score {
    pub fn new(font: u64) -> Result<Self, QPError> {
        Ok(Self { score: 0, font })
    }
}

impl Controller for Score {
    fn update(&mut self, world: &mut World) -> FrameResult {
        let (_x, _y, _width, height) = world.viewport.get_dimensions();
        world.text_buffer.push(qp_gfx::QPText {
            text: format!("score: {}", self.score),
            pos: glm::vec2(20.0, height as f32 - 40.0),
            style: qp_gfx::QPTextStyle {
                font: self.font,
                color: glm::vec4(1.0, 1.0, 1.0, 1.0),
                scale: 0.4,
            },
        });

        FrameResult::None
    }
}

struct GameOver {
    font: u64,
}

impl GameOver {
    pub fn new(font: u64) -> Result<Self, QPError> {
        Ok(Self { font })
    }
}

impl Controller for GameOver {
    fn update(&mut self, world: &mut World) -> FrameResult {
        let (_x, _y, width, height) = world.viewport.get_dimensions();
        world.text_buffer.push(qp_gfx::QPText {
            text: "Game Over".into(),
            pos: glm::vec2((width as f32 / 2.0) - 300.0, height as f32 / 2.0),
            style: qp_gfx::QPTextStyle {
                font: self.font,
                color: glm::vec4(1.0, 1.0, 1.0, 0.6),
                scale: 2.0,
            },
        });
        world.text_buffer.push(qp_gfx::QPText {
            text: "Press Enter to start again".into(),
            pos: glm::vec2((width as f32 / 2.0) - 160.0, (height as f32 / 2.0) - 50.0),
            style: qp_gfx::QPTextStyle {
                font: self.font,
                color: glm::vec4(0.8, 0.8, 0.8, 1.0),
                scale: 0.5,
            },
        });

        FrameResult::None
    }
}

struct DebugInfoText {
    font: u64,
}

impl DebugInfoText {
    pub fn new(font: u64) -> Result<Self, QPError> {
        Ok(Self { font })
    }
}

impl Controller for DebugInfoText {
    fn update(&mut self, world: &mut World) -> FrameResult {
        let entity_count = world.registry.entity_manager.count();
        world.text_buffer.push(qp_gfx::QPText {
            text: format!("entities: {}", entity_count),
            pos: glm::vec2(20.0, 20.0),
            style: qp_gfx::QPTextStyle {
                font: self.font,
                color: glm::vec4(1.0, 1.0, 1.0, 1.0),
                scale: 0.4,
            },
        });

        FrameResult::None
    }
}
