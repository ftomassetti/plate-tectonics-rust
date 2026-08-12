//! Plate velocity, rotation and collision response.

use crate::geometry::{FloatVector, WorldDimension};
use crate::mass::{Mass, MassLike};
use crate::platec_assert;
use crate::simplerandom::SimpleRandom;
use crate::utils::PI;

/// Height limit that separates seas from dry land.
pub const CONT_BASE: f32 = 1.0;
pub const INITIAL_SPEED_X: f32 = 1.0;
pub const DEFORMATION_WEIGHT: f32 = 2.0;

pub type ContinentId = u32;

pub trait MovementLike {
    fn velocity_unit_vector(&self) -> FloatVector;
    fn dec_impulse(&mut self, delta: FloatVector);
}

/// A plate as `Movement::collide` sees it: mass plus motion.
/// `Movement::collide`
/// takes one of these; `test_movement` supplies a mock.
pub trait PlateLike: MassLike + MovementLike {}

impl<T: MassLike + MovementLike> PlateLike for T {}

#[derive(Clone, Debug)]
pub struct Movement {
    randsource: SimpleRandom,
    world_dimension: WorldDimension,
    /// Plate's velocity.
    velocity: f32,
    /// Direction of rotation: 1 = CCW, -1 = clockwise.
    rot_dir: f32,
    /// X and Y components of the plate's acceleration vector.
    dx: f32,
    dy: f32,
    /// X and Y components of the plate's direction unit vector.
    vx: f32,
    vy: f32,
}

impl Movement {
    /// The random-number handling here is
    /// subtle and load-bearing:
    ///
    /// ```text
    /// Movement::Movement(SimpleRandom randsource, const WorldDimension& worldDimension)
    ///     : _randsource(randsource), ..., rot_dir(randsource.next() % 2 ? 1 : -1), ...
    /// { const float angle = 2.0f * PI * _randsource.next_float(); ... }
    /// ```
    ///
    /// `_randsource` is copy-initialised from the by-value parameter *before*
    /// `rot_dir` draws from that parameter, so both `rot_dir` and the initial
    /// angle consume the **same** underlying random value. The caller's
    /// generator is untouched, and the stored member ends up advanced by
    /// exactly one draw. `test_movement` pins this exactly.
    pub fn new(randsource: SimpleRandom, world_dimension: WorldDimension) -> Self {
        let mut member = randsource;
        let mut param = randsource;
        let rot_dir = if param.next() % 2 != 0 { 1.0f32 } else { -1.0f32 };
        let angle = 2.0f32 * PI * member.next_float();
        Self {
            randsource: member,
            world_dimension,
            velocity: 1.0,
            rot_dir,
            dx: 0.0,
            dy: 0.0,
            vx: angle.cos() * INITIAL_SPEED_X,
            vy: angle.sin() * INITIAL_SPEED_X,
        }
    }

    pub fn apply_friction(&mut self, deformed_mass: f32, mass: f32) {
        if mass == 0.0 {
            self.velocity = 0.0;
            return;
        }
        let mut vel_dec = DEFORMATION_WEIGHT * deformed_mass / mass;
        vel_dec = if vel_dec < self.velocity {
            vel_dec
        } else {
            self.velocity
        };

        // Altering the source variable causes the order of calls to this
        // function to have a difference when it shouldn't! However, it's a hack
        // well worth the outcome. :)
        self.velocity -= vel_dec;
    }

    /// Advance the plate along its trajectory.
    pub fn move_plate(&mut self) {
        // Apply any new impulses to the plate's trajectory.
        self.vx += self.dx;
        self.vy += self.dy;
        self.dx = 0.0;
        self.dy = 0.0;

        // Force direction of plate to be unit vector.
        // Update velocity so that the distance of movement doesn't change.
        let len = (self.vx * self.vx + self.vy * self.vy).sqrt();
        platec_assert!(len > 0.0, "Velocity is zero!");
        // Calculating the inverse length and multiplying changes the output
        // data for maps, so the two divisions are kept as-is.
        self.vx /= len;
        self.vy /= len;
        self.velocity += len - 1.0;
        // Round negative values to zero.
        self.velocity *= if self.velocity > 0.0 { 1.0 } else { 0.0 };

        // Apply some circular motion to the plate. Force the radius of the
        // circle to remain fixed by adjusting angular velocity (which depends
        // on the plate's velocity).
        let world_avg_side =
            (self.world_dimension.get_width() + self.world_dimension.get_height()) / 2;
        let alpha = self.rot_dir * self.velocity / (world_avg_side as f32 * 0.33);
        let alpha_vel = alpha * self.velocity;
        let cos = alpha_vel.cos();
        let sin = alpha_vel.sin();
        let vx = self.vx * cos - self.vy * sin;
        let vy = self.vy * cos + self.vx * sin;
        self.vx = vx;
        self.vy = vy;
    }

    pub fn velocity_vector(&self) -> FloatVector {
        FloatVector::new(self.vx * self.velocity, self.vy * self.velocity)
    }

    pub fn velocity_on_x(&self) -> f32 {
        self.vx * self.velocity
    }

    pub fn velocity_on_y(&self) -> f32 {
        self.vy * self.velocity
    }

    pub fn velocity_on_x_len(&self, length: f32) -> f32 {
        platec_assert!(length >= 0.0, "Negative length makes no sense");
        self.vx * length
    }

    pub fn velocity_on_y_len(&self, length: f32) -> f32 {
        platec_assert!(length >= 0.0, "Negative length makes no sense");
        self.vy * length
    }

    pub fn dot(&self, dx: f32, dy: f32) -> f32 {
        self.vx * dx + self.vy * dy
    }

    pub fn momentum(&self, mass: &Mass) -> f32 {
        mass.get_mass() * self.velocity
    }

    pub fn get_velocity(&self) -> f32 {
        self.velocity
    }

    /// Deprecated; use [`Movement::velocity_unit_vector`].
    pub fn vel_x(&self) -> f32 {
        self.vx
    }

    /// Deprecated; use [`Movement::velocity_unit_vector`].
    pub fn vel_y(&self) -> f32 {
        self.vy
    }

    pub fn dec_dx(&mut self, delta: f32) {
        self.dx -= delta;
    }

    pub fn dec_dy(&mut self, delta: f32) {
        self.dy -= delta;
    }

    pub fn add_impulse(&mut self, impulse: FloatVector) {
        self.dx += impulse.x();
        self.dy += impulse.y();
    }

    /// The generator owned by this movement — `plate` draws from it.
    pub fn randsource_mut(&mut self) -> &mut SimpleRandom {
        &mut self.randsource
    }

    pub fn collide(
        &mut self,
        this_mass: &dyn MassLike,
        other_plate: &mut dyn PlateLike,
        coll_mass: f32,
    ) {
        // Coefficient of restitution: 1 = fully elastic, 0 = stick together.
        let coeff_rest = 0.0f32;
        let mass_centers_distance =
            other_plate.mass_center().to_int() - this_mass.mass_center().to_int();
        let distance = mass_centers_distance.length();
        if distance <= 0.0 {
            return; // Avoid division by zero!
        }

        // Compute relative velocity between plates at the collision point.
        // Because torque is not included, the calculation simplifies to
        // v_ab = v_a - v_b.
        let collision_direction = FloatVector::new(
            mass_centers_distance.x() as f32 / distance,
            mass_centers_distance.y() as f32 / distance,
        );
        let relative_velocity = self.velocity_unit_vector() - other_plate.velocity_unit_vector();

        // Get the dot product of the relative velocity vector and the collision
        // vector, then the projection of v_ab along the collision vector. Note
        // that the vector must be a unit vector!
        let rel_dot_n = collision_direction.dot_product(&relative_velocity);
        if rel_dot_n <= 0.0 {
            return; // Exit if objects are moving away from each other.
        }

        // Calculate the denominator of the impulse: n . n * (1 / m_1 + 1 / m_2).
        // Use the mass of the colliding crust for the "donator" plate.
        // Is this a bug? collisionDirection has length 1 because it's a unit
        // vector — the old code is kept in case a float roundoff would change
        // the map.
        let col_len = collision_direction.length();
        let denom = col_len * col_len * (1.0 / other_plate.get_mass() + 1.0 / coll_mass);

        // Calculate force of impulse.
        let j = -(1.0 + coeff_rest) * rel_dot_n / denom;

        // Compute the final change of trajectory. The plate that is the "giver"
        // of the impulse should receive a force according to its pre-collision
        // mass, not the current mass!
        self.add_impulse(collision_direction * (j / this_mass.get_mass()));
        other_plate
            .dec_impulse(collision_direction * (j / (coll_mass + other_plate.get_mass())));
    }
}

impl MovementLike for Movement {
    fn velocity_unit_vector(&self) -> FloatVector {
        FloatVector::new(self.vx, self.vy)
    }

    fn dec_impulse(&mut self, delta: FloatVector) {
        self.dx -= delta.x();
        self.dy -= delta.y();
    }
}
