use engine::{
    gfx::{
        texture,
        ElementArrayMesh
    },
    VersionedIndex,
    Registry,
    resources::{
        Shader,
        Texture
    },
    components::{
        material::MaterialPart,
        CDirection,
        CPosition,
        CAttenuation,
        CCutoff,
        CModelMatrix,
        CModelNode,
        CMaterial,
        CTransform,
        CRGBA
    },
    entity_builders::camera::build_perspective_camera,
    systems::{
        material,
        mvp_matrices::{
            s_set_model_matrix,
            s_set_projection_matrix,
            s_set_view_matrix
        },
        load_obj::{
            s_load_obj_file,
            ObjectConfig
        },
    }
};

use crate::config;

pub fn create_registry() -> Result<engine::Registry, Box<dyn std::error::Error>> {
    let mut registry = engine::Registry::init()?;

    engine::resources::register_resources(&mut registry);
    engine::components::register_components(&mut registry);

    Ok(registry)
}

pub fn create_crates(
    registry: &mut Registry,
    _shader_id: VersionedIndex,
    _camera_id: VersionedIndex,
    material: CMaterial
) -> Result<Vec<engine::VersionedIndex>, Box<dyn std::error::Error>> {
    // load the object data
    let asset_path = config::asset_path()?.into_os_string().into_string().unwrap();
    let (models_obj, _materials_obj) = s_load_obj_file(format!("{}/objects/crate.obj", asset_path))?;
    let model_configs = ObjectConfig::from_obj(models_obj)?;


    let transforms = [
        (glm::vec3(-1.0, 0.0, 0.0), 0.0),
        (glm::vec3(0.1, 0.0, 0.1), 0.1),
        (glm::vec3(-0.3, 1.0, 0.2), 0.02),

        (glm::vec3(-3.0, 0.0, 2.0), 0.0),
        (glm::vec3(-1.9, 0.0, 2.1), 0.1),
        (glm::vec3(-2.3, 1.0, 2.2), 0.02),

        (glm::vec3(1.0, 0.0, -2.0), 0.0),
        (glm::vec3(2.1, 0.0, -2.1), 0.1),
        (glm::vec3(1.7, 1.0, -2.2), 0.02),
    ];

    let mut entities = vec![];
    for config in model_configs.iter() {
        for transform in transforms.iter() {
            let mesh = ElementArrayMesh::new(&config.indices)?;
            mesh
                .create_vbo_at(&config.points, 0, 3)?
                .create_vbo_at(&config.texture_coords, 2, 2)?;

            let entity = registry.create_entity("crate")?
                .with(CModelNode {
                    mesh: Some(mesh),
                    ..CModelNode::default()
                })?
                .with(CPosition::default())?
                .with(CTransform {
                    translate: Some(transform.0),
                    scale: Some(glm::vec3(0.5, 0.5, 0.5)),
                    rotate: Some(vec![(glm::vec3(0.0, 1.0, 0.0), transform.1)]),
                })?
                .with(CModelMatrix::default())?
                .with(material.clone())?
                .done()?;

            s_set_model_matrix(&entity, registry);

            entities.push(entity);
        }
    }

    Ok(entities)
}

pub fn create_camera(
    registry: &mut engine::Registry,
    width: f32,
    height: f32
) -> Result<engine::VersionedIndex, Box<dyn std::error::Error>> {
    let camera = build_perspective_camera(
        registry,
        (0.0, 1.0, 5.0),
        45.0,
        width / height,
        0.1,
        100.0,
        engine::components::CEulerAngles {
            pitch: 0.0,
            yaw: 90.0,
            roll: 0.0
        }
    )?;

    s_set_view_matrix(&camera, registry);
    s_set_projection_matrix(&camera, registry);

    Ok(camera)
}

pub fn create_texture(
    registry: &mut Registry,
    image_file: &str,
) -> Result<VersionedIndex, Box<dyn std::error::Error>> {
    registry.create_resource(Texture {
        id: texture::from_image(image_file)?,
    })
}

pub fn directional_light(
    registry: &mut Registry,
    obj_shader_id: VersionedIndex,
    model_config: &ObjectConfig
) -> Result<VersionedIndex, Box<dyn std::error::Error>> {
    let shader = registry.get_resource::<Shader>(&obj_shader_id)
        .unwrap()
        .program();

    let mat = CMaterial {
        ambient: MaterialPart::Value(0.05, 0.05, 0.05),
        diffuse: MaterialPart::Value(0.1, 0.1, 0.1),
        specular: MaterialPart::Value(0.5, 0.5, 0.5),
        shininess: 0.0,
        ..CMaterial::default()
    };
    
    let direction = (-0.8, -0.1, -0.1);

    if let Some(ambient) = material::s_get_value(&mat.ambient) {
        shader.set_float_3("dirLight.ambient", ambient);
    }
    if let Some(diffuse) = material::s_get_value(&mat.diffuse) {
        shader.set_float_3("dirLight.diffuse", diffuse);
    }
    if let Some(specular) = material::s_get_value(&mat.specular) {
        shader.set_float_3("dirLight.ambient", specular);
    }
    shader.set_float_3("dirLight.direction", direction);

    let mesh = ElementArrayMesh::new(&model_config.indices)?;
    mesh.create_vbo_at(&model_config.points, 0, 3)?;

    let light = registry.create_entity("light")?
        .with(CDirection {
            x: direction.0,
            y: direction.1,
            z: direction.2
        })?
        .with(CPosition::default())?
        .with(CRGBA { r: 1.0, g: 1.0, b: 1.0, a: 1.0 })?
        .with(CModelNode {
            mesh: Some(mesh),
            ..CModelNode::default()
        })?
        .with(CTransform {
            translate: Some(glm::vec3(7.0, 10.0, 0.0)),
            ..CTransform::default()
        })?
        .with(CModelMatrix::default())?
        .with(mat)?
        .done()?;

    s_set_model_matrix(&light, registry);

    Ok(light)
}

pub fn point_light(
    registry: &mut Registry,
    obj_shader_id: VersionedIndex,
    model_config: &ObjectConfig
) -> Result<VersionedIndex, Box<dyn std::error::Error>> {
    let shader = registry.get_resource::<Shader>(&obj_shader_id)
        .unwrap()
        .program();

    let mat = CMaterial {
        ambient: MaterialPart::Value(1.0, 0.0, 0.0),
        diffuse: MaterialPart::Value(1.0, 0.0, 0.0),
        specular: MaterialPart::Value(1.0, 0.2, 0.2),
        shininess: 0.0,
        ..CMaterial::default()
    };

    let position = CPosition {
        x: 5.0,
        y: 1.0,
        z: 6.0
    };
    let attenuation = CAttenuation {
        constant: 1.0,
        linear: 0.09,
        quadratic: 0.032,
    };

    if let Some(ambient) = material::s_get_value(&mat.ambient) {
        shader.set_float_3("pointLight.ambient", ambient);
    }
    if let Some(diffuse) = material::s_get_value(&mat.diffuse) {
        shader.set_float_3("pointLight.diffuse", diffuse);
    }
    if let Some(specular) = material::s_get_value(&mat.specular) {
        shader.set_float_3("pointLight.ambient", specular);
    }
    shader.set_float_3("pointLight.position", (position.x, position.y, position.z));
    shader.set_float("pointLight.constant", attenuation.constant);
    shader.set_float("pointLight.linear", attenuation.linear);
    shader.set_float("pointLight.quadratic", attenuation.quadratic);

    let mesh = ElementArrayMesh::new(&model_config.indices)?;
    mesh.create_vbo_at(&model_config.points, 0, 3)?;

    let light = registry.create_entity("light")?
        .with(position)?
        .with(attenuation)?
        .with(mat)?
        .with(CRGBA { r: 0.6, g: 0.0, b: 0.0, a: 1.0 })?
        .with(CModelNode {
            mesh: Some(mesh),
            ..CModelNode::default()
        })?
        .with(CTransform {
            translate: Some(glm::vec3(5.0, 1.0, 6.0)),
            scale: Some(glm::vec3(0.2, 0.2, 0.2)),
            ..CTransform::default()
        })?
        .with(CModelMatrix::default())?
        .done()?;

    s_set_model_matrix(&light, registry);

    Ok(light)
}

pub fn spot_light(
    registry: &mut Registry,
    obj_shader_id: VersionedIndex,
    model_config: &ObjectConfig
) -> Result<VersionedIndex, Box<dyn std::error::Error>> {
    let shader = registry.get_resource::<Shader>(&obj_shader_id)
        .unwrap()
        .program();

    let mat = CMaterial {
        ambient: MaterialPart::Value(0.1, 0.1, 0.1),
        diffuse: MaterialPart::Value(0.5, 0.5, 0.5),
        specular: MaterialPart::Value(1.0, 1.0, 1.0),
        shininess: 0.0,
        ..CMaterial::default()
    };

    let attenuation = CAttenuation {
        constant: 1.0,
        linear: 0.09,
        quadratic: 0.032,
    };

    let cutoffs = CCutoff {
        inner_cutoff: 12.5_f32.to_radians().cos(),
        outer_cutoff: 17.5_f32.to_radians().cos()
    };

    if let Some(ambient) = material::s_get_value(&mat.ambient) {
        shader.set_float_3("spotLight.ambient", ambient);
    }
    if let Some(diffuse) = material::s_get_value(&mat.diffuse) {
        shader.set_float_3("spotLight.diffuse", diffuse);
    }
    if let Some(specular) = material::s_get_value(&mat.specular) {
        shader.set_float_3("spotLight.ambient", specular);
    }
    shader.set_float("spotLight.constant", attenuation.constant);
    shader.set_float("spotLight.linear", attenuation.linear);
    shader.set_float("spotLight.quadratic", attenuation.quadratic);
    shader.set_float("spotLight.cutOff", cutoffs.inner_cutoff);
    shader.set_float("spotLight.outerCutOff", cutoffs.outer_cutoff);

    let mesh = ElementArrayMesh::new(&model_config.indices)?;
    mesh.create_vbo_at(&model_config.points, 0, 3)?;

    registry.create_entity("light")?
        .with(CRGBA { r: 0.6, g: 0.0, b: 0.0, a: 1.0 })?
        .with(CPosition { x: 0.0, y: 0.0, z: 0.0 })?
        .with(CDirection { x: 0.0, y: 0.0, z: 0.0 })?
        .with(attenuation)?
        .with(cutoffs)?
        .with(mat)?
        .with(CModelNode {
            mesh: Some(mesh),
            ..CModelNode::default()
        })?
        .with(CTransform {
            translate: Some(glm::vec3(5.0, 1.0, 6.0)),
            scale: Some(glm::vec3(0.2, 0.2, 0.2)),
            ..CTransform::default()
        })?
        .done()
}
