// SPDX-License-Identifier: Apache-2.0 OR MIT

use service_people::Person;
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

#[derive(Debug)]
pub enum PeopleStoreError {
    NotFound(String),
    ConflictingIdentity { email: String, existing_id: Uuid, new_id: Uuid },
}

impl std::fmt::Display for PeopleStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeopleStoreError::NotFound(query) => write!(f, "Person not found: {}", query),
            PeopleStoreError::ConflictingIdentity { email, existing_id, new_id } => {
                write!(f, "Email {} already bound to {}, cannot rebind to {}", email, existing_id, new_id)
            }
        }
    }
}

impl std::error::Error for PeopleStoreError {}

pub struct PeopleStore {
    by_email: RwLock<HashMap<String, Person>>,
    by_id: RwLock<HashMap<Uuid, Person>>,
}

impl PeopleStore {
    pub fn new() -> Self {
        Self {
            by_email: RwLock::new(HashMap::new()),
            by_id: RwLock::new(HashMap::new()),
        }
    }

    pub fn append(&self, person: Person) -> Result<(), PeopleStoreError> {
        let normalized_email = person.primary_email.to_lowercase();

        // Check if this email is already bound to a different UUID
        {
            let by_email = self.by_email.read().unwrap();
            if let Some(existing) = by_email.get(&normalized_email) {
                if existing.id != person.id {
                    return Err(PeopleStoreError::ConflictingIdentity {
                        email: normalized_email,
                        existing_id: existing.id,
                        new_id: person.id,
                    });
                }
            }
        }

        // Index by primary email and all aliases
        {
            let mut by_email = self.by_email.write().unwrap();
            by_email.insert(normalized_email, person.clone());
            for alias in &person.email_aliases {
                let normalized_alias = alias.to_lowercase();
                by_email.insert(normalized_alias, person.clone());
            }
        }

        // Index by UUID
        {
            let mut by_id = self.by_id.write().unwrap();
            by_id.insert(person.id, person);
        }

        Ok(())
    }

    pub fn lookup_by_email(&self, email: &str) -> Result<Person, PeopleStoreError> {
        let normalized = email.to_lowercase();
        let by_email = self.by_email.read().unwrap();
        by_email
            .get(&normalized)
            .cloned()
            .ok_or_else(|| PeopleStoreError::NotFound(email.to_string()))
    }

    pub fn lookup_by_id(&self, id: Uuid) -> Result<Person, PeopleStoreError> {
        let by_id = self.by_id.read().unwrap();
        by_id
            .get(&id)
            .cloned()
            .ok_or_else(|| PeopleStoreError::NotFound(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_lookup_by_email() {
        let store = PeopleStore::new();
        let person = Person::new("Alice", "alice@example.com");
        store.append(person.clone()).unwrap();

        let found = store.lookup_by_email("alice@example.com").unwrap();
        assert_eq!(found.id, person.id);
        assert_eq!(found.name, "Alice");
    }

    #[test]
    fn lookup_by_email_case_insensitive() {
        let store = PeopleStore::new();
        let person = Person::new("Bob", "bob@example.com");
        store.append(person.clone()).unwrap();

        let found = store.lookup_by_email("BOB@EXAMPLE.COM").unwrap();
        assert_eq!(found.id, person.id);
    }

    #[test]
    fn lookup_by_alias() {
        let store = PeopleStore::new();
        let person = Person::new("Carol", "carol@work.com")
            .with_alias("carol@personal.com");
        store.append(person.clone()).unwrap();

        let found = store.lookup_by_email("carol@personal.com").unwrap();
        assert_eq!(found.id, person.id);
    }

    #[test]
    fn lookup_by_id() {
        let store = PeopleStore::new();
        let person = Person::new("David", "david@example.com");
        let person_id = person.id;
        store.append(person.clone()).unwrap();

        let found = store.lookup_by_id(person_id).unwrap();
        assert_eq!(found.id, person_id);
    }

    #[test]
    fn lookup_nonexistent_returns_error() {
        let store = PeopleStore::new();
        let result = store.lookup_by_email("nobody@example.com");
        assert!(result.is_err());
    }

    #[test]
    fn conflicting_identity_rejected() {
        let store = PeopleStore::new();
        let person1 = Person::new("Eve", "eve@example.com");
        let mut person2 = Person::new("Eve2", "eve@example.com");
        person2.id = uuid::Uuid::new_v4(); // Force different ID

        store.append(person1).unwrap();
        let result = store.append(person2);
        assert!(result.is_err());
    }
}
