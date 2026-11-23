mod workspace;

use incular::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let search = TextEditingController::new();
    let prompt = TextEditingController::new();
    let app = Application::new(move |_cx| {
        Directionality::new(
            TextDirection::Ltr,
            workspace::view(search.clone(), prompt.clone()),
        )
        .into()
    })?;

    incular::run(app)?;
    Ok(())
}
