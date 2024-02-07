use crate::{
    Registry,
    VersionedIndex,
    components::{
        CGizmo3D,
        CTarget,
        CEulerAngles,
        CDistance,
        CTransform
    }
};

/**
* apply velocity vector to position
*
* requires the following components:
* - CGizmo3D
* - CPosition
*/
pub fn s_apply_velocity(
    registry: &mut Registry,
    entity: &VersionedIndex,
    delta: f32,
    velocity: glm::Vec3
) -> Result<(), Box<dyn std::error::Error>> {
    if let (Some(gizmo), Some(_)) = (
        registry.entities.get::<CGizmo3D>(entity),
        registry.entities.get::<CTransform>(entity)
    ) {
        let mut change_vec = glm::vec3(0.0, 0.0, 0.0);

        change_vec += gizmo.front * velocity.z * delta;
        change_vec += gizmo.up * velocity.y * delta;
        change_vec += gizmo.right * velocity.x * delta;

        let transform = registry.entities.get_mut::<CTransform>(entity).unwrap();
        transform.translate.x += change_vec.x;
        transform.translate.y += change_vec.y;
        transform.translate.z += change_vec.z;
    }

    Ok(())
}

pub fn s_apply_follow_target(
    registry: &mut Registry,
    entity: &VersionedIndex
) -> Result<(), Box<dyn std::error::Error>> {
    if let (Some(_), Some(distance), Some(target), Some(angles)) = (
        registry.entities.get::<CTransform>(entity),
        registry.entities.get::<CDistance>(entity),
        registry.entities.get::<CTarget>(entity),
        registry.entities.get::<CEulerAngles>(entity),
    ) {
        let pos = glm::vec3(
            target.x + distance.0 * angles.yaw.cos() * angles.pitch.sin(),
            target.y + distance.0 * angles.pitch.cos(),
            target.z + distance.0 * angles.yaw.sin() * angles.pitch.sin()
        );

        let transform = registry.entities.get_mut::<CTransform>(entity).unwrap();
        transform.translate.x = pos.x;
        transform.translate.y = pos.y;
        transform.translate.z = pos.z;
    }

    Ok(())

}
