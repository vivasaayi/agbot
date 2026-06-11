use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationRecord {
    pub org_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    Active,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRecord {
    pub user_id: Uuid,
    pub email: String,
    pub org_id: Uuid,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipRecord {
    pub membership_id: Uuid,
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedUserMembership {
    pub user: UserRecord,
    pub membership: MembershipRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ControlPlaneError {
    #[error("organization name cannot be empty")]
    EmptyOrganizationName,
    #[error("organization not found: {org_id}")]
    OrganizationNotFound { org_id: Uuid },
    #[error("email cannot be empty")]
    EmptyEmail,
    #[error("duplicate user email: {email}")]
    DuplicateEmail { email: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlPlaneRegistry {
    organizations: HashMap<Uuid, OrganizationRecord>,
    users: HashMap<Uuid, UserRecord>,
    memberships: HashMap<Uuid, MembershipRecord>,
    email_index: HashMap<String, Uuid>,
}

impl ControlPlaneRegistry {
    pub fn create_organization(
        &mut self,
        name: String,
    ) -> Result<OrganizationRecord, ControlPlaneError> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(ControlPlaneError::EmptyOrganizationName);
        }

        let organization = OrganizationRecord {
            org_id: Uuid::new_v4(),
            name,
            created_at: Utc::now(),
        };
        self.organizations
            .insert(organization.org_id, organization.clone());
        Ok(organization)
    }

    pub fn create_user(
        &mut self,
        org_id: Uuid,
        email: String,
    ) -> Result<CreatedUserMembership, ControlPlaneError> {
        if !self.organizations.contains_key(&org_id) {
            return Err(ControlPlaneError::OrganizationNotFound { org_id });
        }

        let email = normalize_email(&email)?;
        if self.email_index.contains_key(&email) {
            return Err(ControlPlaneError::DuplicateEmail { email });
        }

        let now = Utc::now();
        let user = UserRecord {
            user_id: Uuid::new_v4(),
            email: email.clone(),
            org_id,
            status: UserStatus::Active,
            created_at: now,
        };
        let membership = MembershipRecord {
            membership_id: Uuid::new_v4(),
            user_id: user.user_id,
            org_id,
            joined_at: now,
        };

        self.email_index.insert(email, user.user_id);
        self.users.insert(user.user_id, user.clone());
        self.memberships
            .insert(membership.membership_id, membership.clone());

        Ok(CreatedUserMembership { user, membership })
    }

    pub fn get_organization(&self, org_id: Uuid) -> Option<&OrganizationRecord> {
        self.organizations.get(&org_id)
    }

    pub fn organizations(&self) -> Vec<OrganizationRecord> {
        let mut organizations = self.organizations.values().cloned().collect::<Vec<_>>();
        organizations.sort_by(|left, right| left.name.cmp(&right.name));
        organizations
    }

    pub fn get_user(&self, user_id: Uuid) -> Option<&UserRecord> {
        self.users.get(&user_id)
    }

    pub fn users(&self) -> Vec<UserRecord> {
        let mut users = self.users.values().cloned().collect::<Vec<_>>();
        users.sort_by(|left, right| left.email.cmp(&right.email));
        users
    }

    pub fn memberships_for_org(&self, org_id: Uuid) -> Vec<MembershipRecord> {
        let mut memberships = self
            .memberships
            .values()
            .filter(|membership| membership.org_id == org_id)
            .cloned()
            .collect::<Vec<_>>();
        memberships.sort_by(|left, right| left.joined_at.cmp(&right.joined_at));
        memberships
    }
}

fn normalize_email(email: &str) -> Result<String, ControlPlaneError> {
    let email = email.trim().to_ascii_lowercase();
    if email.is_empty() {
        return Err(ControlPlaneError::EmptyEmail);
    }
    Ok(email)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organization_user_and_membership_are_linked() {
        let mut registry = ControlPlaneRegistry::default();

        let org = registry
            .create_organization("AgBot Farms".to_string())
            .expect("org creates");
        let created = registry
            .create_user(org.org_id, "ops@example.com".to_string())
            .expect("user creates");

        assert_eq!(created.user.email, "ops@example.com");
        assert_eq!(created.user.org_id, org.org_id);
        assert_eq!(created.user.status, UserStatus::Active);
        assert_eq!(created.membership.org_id, org.org_id);
        assert_eq!(created.membership.user_id, created.user.user_id);
        assert!(registry.get_organization(org.org_id).is_some());
        assert_eq!(registry.organizations().len(), 1);
        assert!(registry.get_user(created.user.user_id).is_some());
        assert_eq!(registry.memberships_for_org(org.org_id).len(), 1);
    }

    #[test]
    fn duplicate_email_is_rejected_without_writing_user() {
        let mut registry = ControlPlaneRegistry::default();
        let org = registry
            .create_organization("AgBot Farms".to_string())
            .expect("org creates");
        registry
            .create_user(org.org_id, "ops@example.com".to_string())
            .expect("first user creates");

        let error = registry
            .create_user(org.org_id, "OPS@example.com".to_string())
            .expect_err("duplicate email is rejected case-insensitively");

        assert_eq!(
            error,
            ControlPlaneError::DuplicateEmail {
                email: "ops@example.com".to_string()
            }
        );
        assert_eq!(registry.users().len(), 1);
        assert_eq!(registry.memberships_for_org(org.org_id).len(), 1);
    }

    #[test]
    fn control_plane_records_round_trip_for_api_contract() {
        let mut registry = ControlPlaneRegistry::default();
        let org = registry
            .create_organization("AgBot Farms".to_string())
            .expect("org creates");
        let created = registry
            .create_user(org.org_id, "ops@example.com".to_string())
            .expect("user creates");

        let value = serde_json::to_value(&created).expect("created user serializes");

        assert_eq!(value["user"]["email"], "ops@example.com");
        assert_eq!(value["user"]["org_id"], org.org_id.to_string());
        assert_eq!(value["membership"]["org_id"], org.org_id.to_string());
    }
}
