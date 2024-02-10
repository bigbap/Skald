use quipi::{
    components::{
        CBoundingBox,
        CTransform,
        CVelocity,
        CRGBA
    },
    math::random::Random,
    schemas::{
        ISchema,
        SchemaEntity2D
    },
    utils::now_secs,
    Registry,
    VersionedIndex
};

pub struct RectSpawner {
    camera: VersionedIndex,
    rand: Random
}

impl RectSpawner {
    pub fn new(camera: VersionedIndex) -> Result<RectSpawner, Box<dyn std::error::Error>> {
        Ok(Self {
            camera,
            rand: Random::from_seed(now_secs()?)
        })
    }

    pub fn spawn(
        &mut self,
        registry: &mut Registry,
    ) -> Result<Option<VersionedIndex>, Box<dyn std::error::Error>> {
        let (Some(b_box), mut this_schema) = (
            registry.entities.get::<CBoundingBox>(&self.camera),
            SchemaEntity2D::default()
         ) else {
            return Ok(None);
        };

        let mut vel = (
            self.rand.range(0, 200) as f32,
            self.rand.range(0, 200) as f32,
        );
        if self.rand.random() > 0.5 { vel.0 *= -1.0; }
        if self.rand.random() > 0.5 { vel.1 *= -1.0; }

        this_schema.velocity = Some(CVelocity { x: vel.0, y: vel.1, z: 0.0 });
        this_schema.color = Some(CRGBA {
            r: self.rand.random(),
            g: self.rand.random(),
            b: self.rand.random(),
            a: 0.5
        });

        let s_factor = self.rand.range(25, 50) as f32 / 100.0;
        this_schema.transform = CTransform {
            translate: glm::vec3(
                b_box.right / 2.0,
                b_box.top / 2.0,
                0.0
            ),
            scale: glm::vec3(s_factor, s_factor, s_factor),
            ..CTransform::default()
        };
        this_schema.b_box = Some(CBoundingBox {
            right: 200.0,
            bottom: 200.0,
            ..CBoundingBox::default()
        });

        let id = this_schema.build(registry)?;

        Ok(Some(id))
    }
}
